-- Receives fresh output from emit.rs, rather than keeping a generated snapshot.
local fs = require("@lune/fs")
local serde = require("@lune/serde")
local files = serde.decode("json", fs.readFile(require("@lune/process").args[1]))
local fixture = require("./support/runtime")
local count = 0
local function setup(server)
    local f = fixture(server, true)
    f.packages = f.node("Folder", "Packages", f.services.ReplicatedStorage)
    f.package.Parent = f.packages
    f.shared = f.node("Folder", "RoGridGenerated", f.services.ReplicatedStorage)
    local roots = {
        shared = f.shared,
        server = f.node("Folder", "RoGridGenerated", f.services.ServerScriptService),
        client = f.node("Folder", "RoGridGenerated", f.playerScripts),
    }
    for path, source in files do
        local side, name = string.match(path, "^(%w+)/(%w+)%.luau$")
        f.node("ModuleScript", name, roots[side], source)
    end
    local receiverSource = [[
        local receivers = {
            receive = function() end,
            pose = function() end,
            wide = function(...)
                local offset = if game:GetService("RunService"):IsServer() then 1 else 0
                local value = select(1 + offset, ...)
                for i = 1, 5000 do assert(value["f" .. i] == i) end
                script:SetAttribute("WideCalls", (script:GetAttribute("WideCalls") or 0) + 1)
            end,
            deep = function(...)
                local offset = if game:GetService("RunService"):IsServer() then 1 else 0
                local value = select(1 + offset, ...)
                for _ = 1, 128 do value = value.child end
                assert(value == 42)
                script:SetAttribute("DeepCalls", (script:GetAttribute("DeepCalls") or 0) + 1)
            end,
            many = function(...)
                local offset = if game:GetService("RunService"):IsServer() then 1 else 0
                assert(select("#", ...) == 32 + offset)
                for i = 1, 32 do assert(select(i + offset, ...) == i) end
                script:SetAttribute("Calls", (script:GetAttribute("Calls") or 0) + 1)
            end,
        }
        if game:GetService("RunService"):IsServer() then
            receivers.read = function(_player, value, note) return value, note end
        end
        return receivers
    ]]
    f.receiver = f.node("ModuleScript", "Tests", if server then f.services.ServerStorage else f.playerScripts, receiverSource)
    f.api = f.require(f.package)
    f.caller = roots[if server then "server" else "shared"]:FindFirstChild(if server then "Client" else "Server")
    if not server then
        local definitions = f.shared:FindFirstChild("Definitions")
        f.published("test", f.require(definitions))
        -- The replicated remote tree comes from another process; client modules load during startup.
        f.forget(definitions)
        f.forget(f.shared:FindFirstChild("Revision"))
    end
    return f
end
local function rejects(expected, run)
    local ok, message = pcall(run)
    assert(not ok and string.find(tostring(message), expected, 1, true), `expected {expected}, got {message}`)
    count += 1
end
for _, server in { true, false } do
    local f = setup(server)
    f.api.start()
    local callers = f.require(f.caller)
    if server then
        callers.Tests.receive.fire(f.player)
        callers.Tests.receive.fireAll()
        assert(f.sent[1].method == "FireClient" and f.sent[1].args[1] == f.player)
        assert(f.sent[2].method == "FireAllClients" and f.sent[2].args.n == 0)
    else
        callers.Tests.receive.fire()
        assert(f.sent[1].method == "FireServer" and f.sent[1].args.n == 0)
    end
    count += 1
    if server then
        f.remote("server.Tests.read").OnServerEvent:fire(f.player, "request", 42, nil)
        local response = f.sent[#f.sent]
        assert(response.args.n == 5 and response.args[1] == f.player and response.args[2] == "request")
        assert(response.args[3] == "ok" and response.args[4] == 42 and response.args[5] == nil)
        callers.Tests.pose.fire(f.player, 42)
        callers.Tests.pose.fireAll(42)
    else
        local result
        f.spawn(function() result = table.pack(callers.Tests.read.invoke(42, nil, 5)) end)
        local sent = f.sent[#f.sent]
        assert(sent.args.n == 3 and sent.args[2] == 42 and sent.args[3] == nil)
        sent.remote.OnClientEvent:fire(sent.args[1], "ok", 42, nil)
        f.advance(0)
        assert(result.n == 2 and result[1] == 42 and result[2] == nil and f.timers() == 0)
        callers.Tests.pose.fire(42)
    end
    assert(f.sent[#f.sent].remote.ClassName == "UnreliableRemoteEvent")
    table.clear(f.sent)
    -- Event checks below start with their own outbound calls.
    if server then callers.Tests.receive.fire(f.player); callers.Tests.receive.fireAll()
    else callers.Tests.receive.fire() end
    count += 1
    local values = {}; for i = 1, 32 do values[i] = i end
    local function assertSent(sent, offset)
        assert(sent.args.n == 32 + offset)
        for i = 1, 32 do assert(sent.args[i + offset] == i) end
        if offset == 1 then assert(sent.args[1] == f.player) end
    end
    if server then
        callers.Tests.many.fire(f.player, table.unpack(values))
        callers.Tests.many.fireAll(table.unpack(values))
        assertSent(f.sent[3], 1)
        assertSent(f.sent[4], 0)
    else
        callers.Tests.many.fire(table.unpack(values))
        assertSent(f.sent[2], 0)
    end
    local function inbound()
        if server then f.remote("server.Tests.many").OnServerEvent:fire(f.player, table.unpack(values))
        else f.remote("client.Tests.many").OnClientEvent:fire(table.unpack(values)) end
    end
    inbound()
    assert(f.receiver:GetAttribute("Calls") == 1 and #f.warnings == 0)
    values[32] = "wrong"
    inbound()
    assert(f.receiver:GetAttribute("Calls") == 1 and #f.warnings == 1)
    assert(string.find(f.warnings[1], "p32: expected number", 1, true))
    count += 1
    local leaf = { child = 42 }
    local nested = leaf
    for _ = 2, 128 do nested = { child = nested } end
    if server then callers.Tests.deep.fire(f.player, nested); callers.Tests.deep.fireAll(nested)
    else callers.Tests.deep.fire(nested) end
    local function inboundDeep()
        if server then f.remote("server.Tests.deep").OnServerEvent:fire(f.player, nested)
        else f.remote("client.Tests.deep").OnClientEvent:fire(nested) end
    end
    inboundDeep()
    assert(f.receiver:GetAttribute("DeepCalls") == 1 and #f.warnings == 1)
    leaf.child = "wrong"
    inboundDeep()
    assert(f.receiver:GetAttribute("DeepCalls") == 1 and #f.warnings == 2)
    assert(string.find(f.warnings[2], "expected number", 1, true))
    count += 1
    local wide = {}; for i = 1, 5000 do wide["f" .. i] = i end
    if server then callers.Tests.wide.fire(f.player, wide); callers.Tests.wide.fireAll(wide)
    else callers.Tests.wide.fire(wide) end
    local function inboundWide()
        if server then f.remote("server.Tests.wide").OnServerEvent:fire(f.player, wide)
        else f.remote("client.Tests.wide").OnClientEvent:fire(wide) end
    end
    inboundWide()
    assert(f.receiver:GetAttribute("WideCalls") == 1 and #f.warnings == 2)
    wide.f5000 = "wrong"
    inboundWide()
    assert(f.receiver:GetAttribute("WideCalls") == 1 and #f.warnings == 3)
    assert(string.find(f.warnings[3], "value.f5000: expected number", 1, true))
    count += 1
    rejects("only once", f.api.start)
    for _, scenario in { "protocol", "revision", "missing revision", "wrong revision class", "missing package", "wrong package class", "missing packages", "missing shared" } do
        f = setup(server)
        local expected
        local missing
        if scenario == "protocol" then f.api._protocol = 0; expected = "CLI and package are incompatible"
        elseif scenario == "revision" then f.shared:FindFirstChild("Revision").Source = "return 'old'"; expected = "generated files disagree"
        elseif scenario == "missing revision" then missing = f.shared:FindFirstChild("Revision")
        elseif scenario == "wrong revision class" then f.shared:FindFirstChild("Revision").ClassName = "Folder"; expected = "revision must be a ModuleScript"
        elseif scenario == "missing package" then missing = f.package
        elseif scenario == "wrong package class" then f.package.ClassName = "Folder"; expected = "package must be a ModuleScript"
        elseif scenario == "missing packages" then missing = f.packages
        else missing = f.shared end
        if missing then
            local parent = missing.Parent
            missing.Parent = nil
            local loading = coroutine.create(function() return f.require(f.caller) end)
            f.waiting(loading, parent, missing.Name)
            missing.Parent = parent
            assert(typeof(f.completed(loading).Tests.receive.fire) == "function")
            count += 1
        else
            rejects(expected, function() f.require(f.caller) end)
        end
    end
end
print(`Generated runtime: {count} checks passed`)
