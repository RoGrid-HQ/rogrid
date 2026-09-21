-- Minimal engine stand-ins with cooperative waits for testing the real runtime.
-- Network delivery and replication are exercised separately in Roblox Studio.
local fs = require("@lune/fs")
local luau = require("@lune/luau")
local validate = require("../../src/validate")

return function(server, studio)
    local fixture = { warnings = {}, nodes = {}, sent = {}, now = 0, definitions = {}, bound = {} }
    local scheduled = {}
    local cancelled = {}
    local function schedule(callback, delay, ...)
        local thread = if typeof(callback) == "thread" then callback else coroutine.create(callback)
        table.insert(scheduled, { thread = thread, at = fixture.now + delay, args = table.pack(...) })
        return thread
    end
    fixture.task = {
        defer = function(callback, ...) return schedule(callback, 0, ...) end,
        delay = function(delay, callback, ...) return schedule(callback, delay, ...) end,
        cancel = function(thread) cancelled[thread] = true end,
        wait = function(delay)
            schedule(coroutine.running(), delay or 0)
            return coroutine.yield()
        end,
    }
    function fixture.spawn(callback)
        local thread = coroutine.create(callback)
        local ok, message = coroutine.resume(thread)
        assert(ok, message)
        return thread
    end
    function fixture.advance(seconds)
        fixture.now += seconds
        while true do
            local index
            for i, job in scheduled do
                if cancelled[job.thread] or job.at <= fixture.now then index = i; break end
            end
            if not index then break end
            local job = table.remove(scheduled, index)
            if not cancelled[job.thread] then
                local ok, message = coroutine.resume(job.thread, table.unpack(job.args, 1, job.args.n))
                assert(ok, message)
            end
        end
    end
    function fixture.timers()
        local count = 0
        for _, job in scheduled do if not cancelled[job.thread] then count += 1 end end
        return count
    end
    local function signal()
        local listeners = {}
        return {
            Connect = function(_, callback) table.insert(listeners, callback); return { Disconnect = function() end } end,
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
            local started = fixture.now
            while true do
                local child = self:FindFirstChild(childName)
                if child then return child end
                if timeout and fixture.now - started >= timeout then return nil end
                coroutine.yield(self, childName)
            end
        end
        function node:SetAttribute(key, value) self.attributes[key] = value end
        function node:GetAttribute(key) return self.attributes[key] end
        if class == "RemoteEvent" or class == "UnreliableRemoteEvent" then
            node.OnServerEvent = signal()
            node.OnClientEvent = signal()
            for _, method in { "FireServer", "FireClient", "FireAllClients" } do
                node[method] = function(_, ...)
                    local sent = { method = method, remote = node, args = table.pack(...) }
                    table.insert(fixture.sent, sent)
                    if fixture.onSend then fixture.onSend(sent) end
                end
            end
        end
        table.insert(fixture.nodes, node)
        return node
    end
    -- Missing startup dependencies remain awaited as simulated time advances.
    function fixture.waiting(thread, parent, name)
        for _, elapsed in { 0, 60 } do
            fixture.now += elapsed
            local ok, actualParent, actualName = coroutine.resume(thread)
            assert(ok, actualParent)
            assert(coroutine.status(thread) == "suspended" and actualParent == parent and actualName == name,
                `expected to keep waiting for {name}`)
        end
    end
    function fixture.completed(thread)
        local ok, result = coroutine.resume(thread)
        assert(ok, result)
        assert(coroutine.status(thread) == "dead", "expected startup to finish")
        return result
    end
    fixture.services = {}
    for _, name in { "Players", "ReplicatedStorage", "ServerStorage", "ServerScriptService" } do
        fixture.services[name] = fixture.node(name)
    end
    fixture.services.RunService = { IsServer = function() return server end, IsClient = function() return not server end, IsStudio = function() return studio end }
    fixture.services.Players.PlayerRemoving = signal()
    local nextId = 0
    fixture.services.HttpService = { GenerateGUID = function() nextId += 1; return "request-" .. nextId end }
    fixture.player = fixture.node("Player", "Player", fixture.services.Players)
    fixture.services.Players.LocalPlayer = fixture.player
    fixture.playerScripts = fixture.node("PlayerScripts", "PlayerScripts", fixture.player)
    fixture.package = fixture.node("ModuleScript", "rogrid", nil, fs.readFile("packages/rogrid/src/init.luau"))
    fixture.runtimeModule = fixture.node("ModuleScript", "runtime", fixture.package, fs.readFile("packages/rogrid/src/runtime.luau"))
    fixture.validator = fixture.node("ModuleScript", "validate", fixture.package)
    fixture.package.validate = fixture.validator
    for _, name in { "transport", "requests" } do
        fixture.package[name] = fixture.node("ModuleScript", name, fixture.package, fs.readFile("packages/rogrid/src/" .. name .. ".luau"))
    end
    local cache = {}
    function fixture.forget(module) cache[module] = nil end
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
            task = fixture.task,
            warn = function(message) table.insert(fixture.warnings, message) end,
        }
        local result = luau.load(module.Source, { debugName = module.Name, environment = environment })()
        cache[module] = { result }
        return result
    end
    function fixture.remote(id)
        for _, node in fixture.nodes do if (node.ClassName == "RemoteEvent" or node.ClassName == "UnreliableRemoteEvent") and node.Name == id then return node end end
        error("missing fixture remote " .. id)
    end
    function fixture.events(ids, arguments)
        local definitions = {}
        for _, id in ids do
            table.insert(definitions, { id = id, receiver = string.match(id, "^(%w+)%."), kind = "event", reliability = "reliable", arguments = arguments or {} })
        end
        return definitions
    end
    function fixture.published(revision, definitions)
        local folder = fixture.node("Folder", "RoGridRemotes", fixture.services.ReplicatedStorage)
        folder:SetAttribute("Revision", revision)
        for _, definition in definitions do fixture.node(if definition.reliability == "unreliable" then "UnreliableRemoteEvent" else "RemoteEvent", definition.id, folder) end
        return folder
    end
    function fixture.begin(side, revision, definitions)
        fixture.definitions = definitions
        fixture.runtime.begin(side, revision, definitions)
    end
    function fixture.bind(id, handler, arguments)
        for _, definition in fixture.definitions do
            if definition.id == id and arguments then definition.arguments = arguments end
        end
        fixture.runtime.bind(id, handler)
        fixture.bound[id] = true
    end
    function fixture.finish()
        for _, definition in fixture.definitions do
            if (definition.receiver == "server") == server and not fixture.bound[definition.id] then
                fixture.bind(definition.id, function() end)
            end
        end
        fixture.runtime.finish()
    end
    fixture.runtime = fixture.require(fixture.runtimeModule)
    return fixture
end
