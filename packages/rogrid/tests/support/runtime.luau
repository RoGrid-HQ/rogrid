-- Minimal synchronous engine stand-ins for testing the real runtime's branches.
-- Network delivery and replication are exercised separately in Roblox Studio.
local fs = require("@lune/fs")
local luau = require("@lune/luau")
local validate = require("../../src/validate")

return function(server, studio)
    local fixture = { warnings = {}, waits = {}, nodes = {}, sent = {}, now = 0 }
    local function signal()
        local listeners = {}
        return {
            Connect = function(_, callback) table.insert(listeners, callback) end,
            fire = function(_, ...) for _, callback in listeners do callback(...) end end,
        }
    end
    function fixture.node(class, name, parent, source)
        local node = { ClassName = class, Name = name or class, Parent = parent, Source = source, attributes = {} }
        function node:IsA(expected) return expected == "Instance" or expected == self.ClassName end
        function node:FindFirstChild(childName)
            for _, child in fixture.nodes do
                if child.Parent == self and child.Name == childName then return child end
            end
            return nil
        end
        function node:WaitForChild(childName, timeout)
            table.insert(fixture.waits, { childName, timeout })
            return self:FindFirstChild(childName)
        end
        function node:SetAttribute(key, value) self.attributes[key] = value end
        function node:GetAttribute(key) return self.attributes[key] end
        if class == "RemoteEvent" then
            node.OnServerEvent = signal()
            node.OnClientEvent = signal()
            for _, method in { "FireServer", "FireClient", "FireAllClients" } do
                node[method] = function(_, ...)
                    table.insert(fixture.sent, { method = method, remote = node, args = table.pack(...) })
                end
            end
        end
        table.insert(fixture.nodes, node)
        return node
    end
    fixture.services = {}
    for _, name in { "Players", "ReplicatedStorage", "ServerStorage", "ServerScriptService" } do
        fixture.services[name] = fixture.node(name)
    end
    fixture.services.RunService = { IsServer = function() return server end, IsClient = function() return not server end, IsStudio = function() return studio end }
    fixture.player = fixture.node("Player")
    fixture.services.Players.LocalPlayer = fixture.player
    fixture.playerScripts = fixture.node("PlayerScripts", "PlayerScripts", fixture.player)
    fixture.package = fixture.node("ModuleScript", "rogrid", nil, fs.readFile("packages/rogrid/src/init.luau"))
    fixture.runtimeModule = fixture.node("ModuleScript", "runtime", fixture.package, fs.readFile("packages/rogrid/src/runtime.luau"))
    fixture.validator = fixture.node("ModuleScript", "validate", fixture.package)
    -- Support script.Parent.validate, used by the actual runtime source.
    fixture.package.validate = fixture.validator
    local cache = {}
    function fixture.require(module)
        assert(module and module:IsA("ModuleScript"), "test require expects a ModuleScript")
        if module == fixture.validator then return validate end
        if cache[module] then return cache[module][1] end
        local environment = {
            game = { GetService = function(_, name) return assert(fixture.services[name], name) end },
            script = module,
            Instance = { new = function(class) return fixture.node(class) end },
            require = fixture.require,
            os = { clock = function() return fixture.now end },
            warn = function(message) table.insert(fixture.warnings, message) end,
        }
        local result = luau.load(module.Source, { debugName = module.Name, environment = environment })()
        cache[module] = { result }
        return result
    end
    function fixture.remote(id)
        for _, node in fixture.nodes do if node.ClassName == "RemoteEvent" and node.Name == id then return node end end
        error("missing fixture remote " .. id)
    end
    function fixture.published(revision, ids)
        local folder = fixture.node("Folder", "RoGridEvents", fixture.services.ReplicatedStorage)
        folder:SetAttribute("Revision", revision)
        for _, id in ids do fixture.node("RemoteEvent", id, folder) end
        return folder
    end
    fixture.runtime = fixture.require(fixture.runtimeModule)
    return fixture
end
