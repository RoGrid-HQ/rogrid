local fixture = require("./support/runtime")
local count = 0
local function test(name, run)
    local ok, message = pcall(run)
    assert(ok, `{name}: {message}`)
    count += 1
end
local function rejects(expected, run)
    local ok, message = pcall(run)
    assert(not ok and string.find(tostring(message), expected, 1, true), `expected {expected}, got {message}`)
end
local ids = { "server.Tests.send", "server.Tests.other", "client.Tests.reply" }
local parameters = { { name = "value", shape = "number" } }
local function begun(server, studio)
    local f = fixture(server, studio)
    if not server then f.published("test", ids) end
    f.runtime.begin(if server then "server" else "client", "test", ids)
    return f
end

test("publishes remotes only after registration finishes", function()
    local f = begun(true, true)
    assert(f.services.ReplicatedStorage:FindFirstChild("RoGridEvents") == nil)
    local received = 0
    f.runtime.bind(ids[1], function() received += 1 end, parameters)
    f.remote(ids[1]).OnServerEvent:fire(f.player, 1)
    assert(received == 0)
    f.runtime.finish()
    local folder = f.services.ReplicatedStorage:FindFirstChild("RoGridEvents")
    assert(folder and folder:GetAttribute("Revision") == "test")
    f.remote(ids[1]).OnServerEvent:fire(f.player, 1)
    assert(received == 1)
end)

test("startup and registration lifecycle guards", function()
    local f = fixture(true, true)
    rejects("has not begun", f.runtime.finish)
    rejects("wrong side", function() f.runtime.begin("client", "test", ids) end)
    f.runtime.begin("server", "test", ids)
    rejects("only once", function() f.runtime.begin("server", "test", ids) end)
    rejects("wrong side", function() f.runtime.bind(ids[3], function() end, {}) end)
    rejects("is missing", function() f.runtime.bind("server.Missing.event", function() end, {}) end)
    rejects("before firing events", function() f.runtime.fireAllClients(ids[3], "test", 1) end)
    f.runtime.finish()
    rejects("already started", f.runtime.finish)
    rejects("during startup", function() f.runtime.bind(ids[1], function() end, {}) end)
end)

test("conflicting existing remotes are rejected", function()
    local f = fixture(true, true)
    f.published("test", ids)
    rejects("already exists", function() f.runtime.begin("server", "test", ids) end)
end)

test("client startup detects missing remotes and generation mismatches", function()
    for _, scenario in { "missing", "wrong folder", "revision", "missing remote", "wrong remote" } do
        local f = fixture(false, true)
        if scenario ~= "missing" then
            local folder = f.published(if scenario == "revision" then "old" else "test", ids)
            if scenario == "wrong folder" then folder.ClassName = "Model" end
            if scenario == "missing remote" then f.remote(ids[1]).Parent = nil end
            if scenario == "wrong remote" then f.remote(ids[1]).ClassName = "Folder" end
        end
        local expected = if scenario == "revision" then "generation differs" elseif string.find(scenario, "remote") then "is missing" else "did not start within 10 seconds"
        rejects(expected, function() f.runtime.begin("client", "test", ids) end)
        for _, wait in f.waits do assert(wait[2] == 10) end
    end
end)

test("outbound calls preserve targets arguments and trailing nils", function()
    local f = begun(true, true)
    f.runtime.finish()
    f.runtime.fireClient(ids[3], "test", f.player, 42, nil)
    f.runtime.fireAllClients(ids[3], "test", 7, nil)
    assert(f.sent[1].method == "FireClient" and f.sent[1].remote == f.remote(ids[3]))
    assert(f.sent[1].args.n == 3 and f.sent[1].args[1] == f.player and f.sent[1].args[2] == 42)
    assert(f.sent[2].method == "FireAllClients" and f.sent[2].args.n == 2 and f.sent[2].args[1] == 7)
    local client = begun(false, true)
    client.runtime.finish()
    client.runtime.fireServer(ids[1], "test", 12, nil)
    assert(client.sent[1].method == "FireServer" and client.sent[1].args.n == 2 and client.sent[1].args[1] == 12)
end)

test("wrong sides directions unknown events and stale callers are rejected", function()
    for _, server in { true, false } do
        local f = begun(server, true)
        f.runtime.finish()
        if server then
            rejects("only be used on the client", function() f.runtime.fireServer(ids[1], "test") end)
            rejects("wrong direction", function() f.runtime.fireAllClients(ids[1], "test") end)
            rejects("callers are stale", function() f.runtime.fireClient(ids[3], "old", f.player) end)
            rejects("Unknown RoGrid event", function() f.runtime.fireAllClients("client.Missing.event", "test") end)
        else
            rejects("only be used on the server", function() f.runtime.fireClient(ids[3], "test", f.player) end)
            rejects("only be used on the server", function() f.runtime.fireAllClients(ids[3], "test") end)
            rejects("wrong direction", function() f.runtime.fireServer(ids[3], "test") end)
            rejects("callers are stale", function() f.runtime.fireServer(ids[1], "old") end)
            rejects("Unknown RoGrid event", function() f.runtime.fireServer("server.Missing.event", "test") end)
        end
        assert(#f.sent == 0)
    end
end)

test("invalid inbound payloads are dropped on both sides", function()
    for _, server in { true, false } do
        local f = begun(server, true)
        local id = if server then ids[1] else ids[3]
        local received = {}
        f.runtime.bind(id, function(...) table.insert(received, table.pack(...)) end, parameters)
        f.runtime.finish()
        local function send(...)
            if server then f.remote(id).OnServerEvent:fire(f.player, ...) else f.remote(id).OnClientEvent:fire(...) end
        end
        send("wrong"); send(); send(1, 2)
        assert(#received == 0)
        send(42)
        assert(#received == 1)
        assert(received[1].n == (if server then 2 else 1))
        assert(received[1][if server then 2 else 1] == 42)
        if server then assert(received[1][1] == f.player) end
    end
end)

test("valid bursts have no framework rate limit", function()
    local f = begun(true, true)
    local received = 0
    for _, id in { ids[1], ids[2] } do f.runtime.bind(id, function() received += 1 end, parameters) end
    f.runtime.finish()
    for i = 1, 200 do f.remote(ids[1 + i % 2]).OnServerEvent:fire(f.player, i) end
    assert(received == 200 and f.now == 0)
end)

test("Studio warnings are throttled independently per event", function()
    local f = begun(true, true)
    for _, id in { ids[1], ids[2] } do f.runtime.bind(id, function() error("invalid payload invoked handler") end, parameters) end
    f.runtime.finish()
    f.remote(ids[1]).OnServerEvent:fire(f.player, "wrong")
    f.remote(ids[1]).OnServerEvent:fire(f.player, "wrong")
    f.remote(ids[2]).OnServerEvent:fire(f.player, "wrong")
    assert(#f.warnings == 2)
    assert(string.find(f.warnings[1], "RoGrid dropped " .. ids[1], 1, true))
    assert(string.find(f.warnings[1], "value", 1, true))
    f.now = 0.99; f.remote(ids[1]).OnServerEvent:fire(f.player, "wrong")
    assert(#f.warnings == 2)
    f.now = 1; f.remote(ids[1]).OnServerEvent:fire(f.player, "wrong")
    assert(#f.warnings == 3)
end)

test("production drops invalid payloads silently", function()
    local f = begun(true, false)
    f.runtime.bind(ids[1], function() error("invalid payload invoked handler") end, parameters)
    f.runtime.finish()
    f.remote(ids[1]).OnServerEvent:fire(f.player, "wrong")
    assert(#f.warnings == 0)
end)

test("handler errors include the event and traceback and subsequent messages still run", function()
    for _, server in { true, false } do
        local f = begun(server, false)
        local id = if server then ids[1] else ids[3]
        local received = 0
        f.runtime.bind(id, function()
            received += 1
            if received == 1 then error("handler exploded") end
        end, parameters)
        f.runtime.finish()
        for _ = 1, 2 do
            if server then f.remote(id).OnServerEvent:fire(f.player, 1) else f.remote(id).OnClientEvent:fire(1) end
        end
        assert(received == 2 and #f.warnings == 1)
        assert(string.find(f.warnings[1], id, 1, true) and string.find(f.warnings[1], "handler exploded", 1, true))
        assert(string.find(f.warnings[1], "\n", 1, true))
    end
end)

test("receiver module paths and exports are checked", function()
    for _, server in { true, false } do
        local f = fixture(server, true)
        local root = if server then f.services.ServerStorage else f.playerScripts
        local location = { if server then "ServerStorage" else "PlayerScripts", "Events" }
        rejects("receiver", function() f.runtime.load(location, { "receive" }) end)
        local module = f.node("Folder", "Events", root)
        rejects("must be a ModuleScript", function() f.runtime.load(location, { "receive" }) end)
        module.Parent = nil
        for _, case in {
            { "return 42", "must return a table" },
            { "return {}", "is missing" },
            { "return {receive = 1}", "is missing" },
            { "return {receive = function() end, extra = true}", "ungenerated export" },
        } do
            module = f.node("ModuleScript", "Events", root, case[1])
            rejects(case[2], function() f.runtime.load(location, { "receive" }) end)
            module.Parent = nil
        end
        f.node("ModuleScript", "Events", root, "return {receive = function() return 42 end}")
        assert(f.runtime.load(location, { "receive" }).receive() == 42)
    end
    local f = fixture(true, true)
    rejects("require a client", function() f.runtime.load({ "PlayerScripts", "Events" }, {}) end)
end)

test("public API preserves event handlers and diagnoses absent generated startup", function()
    for _, server in { true, false } do
        local f = fixture(server, true)
        local api = f.require(f.package)
        local handler = function(value) return value end
        assert(api.event(handler) == handler and api.event(handler)(42) == 42)
        rejects("generated files are missing", api.start)
        local shared = f.node("Folder", "RoGridGenerated", f.services.ReplicatedStorage)
        if not server then
            rejects("callers are missing", api.start)
            f.node("ModuleScript", "Server", shared, "return {}")
        end
        local generated = f.node("Folder", "RoGridGenerated", if server then f.services.ServerScriptService else f.playerScripts)
        rejects("startup is missing", api.start)
        f.node("ModuleScript", "Start", generated, "return function() error('startup was invoked') end")
        rejects("startup was invoked", api.start)
    end
end)

test("package entrypoint diagnoses missing runtime", function()
    local f = fixture(true, true)
    f.runtimeModule.Parent = nil
    rejects("runtime is missing", function() f.require(f.package) end)
end)

print(`Runtime: {count} scenarios passed`)
