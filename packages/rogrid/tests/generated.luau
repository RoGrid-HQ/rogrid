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
    f.node("ModuleScript", "Tests", f.services.ServerStorage, "return {receive = function() end}")
    f.node("ModuleScript", "Tests", f.playerScripts, "return {receive = function() end}")
    f.api = f.require(f.package)
    f.caller = roots[if server then "server" else "shared"]:FindFirstChild(if server then "Client" else "Server")
    if not server then f.published("test", { "server.Tests.receive", "client.Tests.receive" }) end
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
    rejects("only once", f.api.start)
    for _, scenario in { "protocol", "revision", "missing revision", "wrong revision class", "missing package", "wrong package class", "missing packages", "missing shared" } do
        f = setup(server)
        local expected
        if scenario == "protocol" then f.api._protocol = 0; expected = "CLI and package are incompatible"
        elseif scenario == "revision" then f.shared:FindFirstChild("Revision").Source = "return 'old'"; expected = "generated files disagree"
        elseif scenario == "missing revision" then f.shared:FindFirstChild("Revision").Parent = nil; expected = "revision is missing"
        elseif scenario == "wrong revision class" then f.shared:FindFirstChild("Revision").ClassName = "Folder"; expected = "revision is missing"
        elseif scenario == "missing package" then f.package.Parent = nil; expected = "package is missing"
        elseif scenario == "wrong package class" then f.package.ClassName = "Folder"; expected = "package is missing"
        elseif scenario == "missing packages" then f.packages.Parent = nil; expected = "packages are missing"
        else f.shared.Parent = nil; expected = "output is missing" end
        rejects(expected, function() f.require(f.caller) end)
    end
end
print(`Generated runtime: {count} checks passed`)
