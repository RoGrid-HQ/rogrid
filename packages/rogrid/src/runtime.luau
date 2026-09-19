--!strict
-- Used by generated code. Public callers keep concrete argument types.
local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local RunService = game:GetService("RunService")
local validate = require(script.Parent.validate)

type Handler = (...any) -> ()

local Runtime = {}
local remotes: { [string]: RemoteEvent } = {}
-- Receiver paths are selected by generated registration, so their return shape
-- is checked below rather than inferred through a statically known require.
local loadModule: (ModuleScript) -> any = require
local root: Folder? = nil
local revision: string? = nil
local side: string? = nil
local started = false

local function invoke(id: string, handler: Handler, ...: any): ()
    local ok, message = xpcall(handler :: (...any) -> any, debug.traceback, ...)
    if not ok then
        warn(`RoGrid receiver {id} failed:\n{message}`)
    end
end

function Runtime.begin(destination: string, expected: string, ids: { string }): ()
    assert(side == nil, "RoGrid.start must run only once per server/client. Restart Play after edits.")
    assert((destination == "server") == RunService:IsServer(), "RoGrid startup ran on the wrong side")
    side = destination
    revision = expected

    local folder: Folder
    if destination == "server" then
        assert(not ReplicatedStorage:FindFirstChild("RoGridEvents"), "RoGridEvents already exists")
        folder = Instance.new("Folder")
        folder.Name = "RoGridEvents"
        folder:SetAttribute("Revision", expected)
        -- Publish only after every server receiver has been connected.
        for _, id in ids do
            local remote = Instance.new("RemoteEvent")
            remote.Name = id
            remote.Parent = folder
            remotes[id] = remote
        end
    else
        local found = ReplicatedStorage:WaitForChild("RoGridEvents", 10)
        assert(found and found:IsA("Folder"), "RoGrid server did not start within 10 seconds")
        assert(found:GetAttribute("Revision") == expected, "RoGrid client/server generation differs; restart Play after running rogrid dev")
        folder = found
        for _, id in ids do
            local remote = folder:WaitForChild(id, 10)
            assert(remote and remote:IsA("RemoteEvent"), `RoGrid event {id} is missing`)
            remotes[id] = remote
        end
    end
    root = folder
end

function Runtime.load(location: { string }, names: { string }): { [string]: Handler }
    local current: Instance
    if location[1] == "PlayerScripts" then
        assert(RunService:IsClient(), "PlayerScripts event paths require a client")
        local scripts = Players.LocalPlayer:WaitForChild("PlayerScripts", 10)
        assert(scripts, "PlayerScripts is missing")
        current = scripts
    else
        current = game:GetService(location[1])
    end
    for index = 2, #location do
        local child = current:WaitForChild(location[index], 10)
        assert(child, `RoGrid receiver {table.concat(location, "/")} is missing; check the Rojo mapping and regenerate`)
        current = child
    end
    local module = current
    local moduleName = table.concat(location, "/")
    assert(module:IsA("ModuleScript"), `RoGrid receiver {moduleName} must be a ModuleScript`)
    local exports = loadModule(module)
    assert(typeof(exports) == "table", `RoGrid module {moduleName} must return a table`)
    local expected: { [string]: boolean } = {}
    for _, name in names do
        expected[name] = true
        assert(typeof(exports[name]) == "function", `RoGrid receiver {moduleName}.{name} is missing; regenerate`)
    end
    for name in exports do
        assert(expected[name], `RoGrid module {moduleName} has an ungenerated export; regenerate`)
    end
    return exports
end

function Runtime.bind(id: string, handler: Handler, parameters: { validate.Parameter }): ()
    assert(not started, "RoGrid receivers must be registered during startup")
    assert(side ~= nil and string.sub(id, 1, #side + 1) == side .. ".", "RoGrid receiver is on the wrong side")
    local remote = remotes[id]
    assert(remote, `RoGrid event {id} is missing`)
    local lastWarning = -math.huge
    local function accepted(...: any): boolean
        local message = validate(parameters, ...)
        if message == nil then return true end
        if RunService:IsStudio() and os.clock() - lastWarning >= 1 then
            lastWarning = os.clock()
            warn(`RoGrid dropped {id}: {message}`)
        end
        return false
    end
    if side == "server" then
        remote.OnServerEvent:Connect(function(player: Player, ...: any)
            if not started or not accepted(...) then
                return
            end
            invoke(id, handler, player, ...)
        end)
    else
        remote.OnClientEvent:Connect(function(...: any)
            if not accepted(...) then
                return
            end
            invoke(id, handler, ...)
        end)
    end
end

function Runtime.finish(): ()
    local folder = root
    assert(folder, "RoGrid startup has not begun")
    assert(not started, "RoGrid has already started")
    started = true
    if side == "server" then
        folder.Parent = ReplicatedStorage
    end
end

local function remoteFor(id: string, expected: string, destination: string): RemoteEvent
    assert(started, "Call RoGrid.start before firing events; do not fire at receiver module top level")
    assert(expected == revision, "RoGrid callers are stale; regenerate and restart Play")
    assert(string.sub(id, 1, #destination + 1) == destination .. ".", "RoGrid event has the wrong direction")
    local remote = remotes[id]
    assert(remote, `Unknown RoGrid event {id}`)
    return remote
end

function Runtime.fireServer(id: string, expected: string, ...: any): ()
    assert(side == "client", "Server event callers can only be used on the client")
    remoteFor(id, expected, "server"):FireServer(...)
end

function Runtime.fireClient(id: string, expected: string, player: Player, ...: any): ()
    assert(side == "server", "Client event callers can only be used on the server")
    remoteFor(id, expected, "client"):FireClient(player, ...)
end

function Runtime.fireAllClients(id: string, expected: string, ...: any): ()
    assert(side == "server", "Client event callers can only be used on the server")
    remoteFor(id, expected, "client"):FireAllClients(...)
end

return Runtime
