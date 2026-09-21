--!strict
-- Shared lifecycle and endpoint dispatch used by generated callers.
local Players = game:GetService("Players")
local RunService = game:GetService("RunService")
local Transport = require(script.Parent.transport)
local Requests = require(script.Parent.requests)
local validate = require(script.Parent.validate)

type Handler = (...any) -> ...any
export type Definition = {
    id: string,
    receiver: "server" | "client",
    kind: "event" | "request",
    reliability: ("reliable" | "unreliable")?,
    arguments: { validate.Parameter },
    returns: { validate.Parameter }?,
}
type Endpoint = { definition: Definition, remote: Transport.Remote, bound: boolean }

local Runtime = {}
local endpoints: { [string]: Endpoint } = {}
local loadModule: (ModuleScript) -> any = require
local root: Folder? = nil
local revision: string? = nil
local side: string? = nil
local started = false
local requests = Requests.new()

Players.PlayerRemoving:Connect(function(player: Player)
    requests.disconnect(if side == "client" and player == Players.LocalPlayer then nil else player)
end)

function Runtime.begin(destination: string, expected: string, definitions: { Definition }): ()
    assert(side == nil, "RoGrid.start must run only once per server/client. Restart Play after edits.")
    assert((destination == "server") == RunService:IsServer(), "RoGrid startup ran on the wrong side")
    side = destination
    revision = expected
    local folder = Transport.open(expected)
    root = folder
    for _, definition in definitions do
        assert(not endpoints[definition.id], `Duplicate RoGrid endpoint {definition.id}`)
        local remote = Transport.remote(folder, definition.id, definition.reliability == "unreliable")
        endpoints[definition.id] = { definition = definition, remote = remote, bound = false }
        if definition.kind == "request" and definition.receiver ~= side then
            assert(remote:IsA("RemoteEvent"), "RoGrid requests require a reliable RemoteEvent")
            Transport.connect(remote, function(peer: Player?, ...: any)
                requests.receive(remote, peer, ...)
            end)
        end
    end
end

function Runtime.load(location: { string }, names: { string }): { [string]: Handler }
    local current: Instance
    if location[1] == "PlayerScripts" then
        assert(RunService:IsClient(), "PlayerScripts event paths require a client")
        current = Players.LocalPlayer:WaitForChild("PlayerScripts")
    else
        current = game:GetService(location[1])
    end
    for index = 2, #location do
        current = current:WaitForChild(location[index])
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

function Runtime.bind(id: string, handler: Handler): ()
    assert(not started, "RoGrid receivers must be registered during startup")
    local endpoint = endpoints[id]
    assert(endpoint, `RoGrid endpoint {id} is missing`)
    local definition = endpoint.definition
    assert(side ~= nil and definition.receiver == side, "RoGrid receiver is on the wrong side")
    assert(not endpoint.bound, `RoGrid receiver {id} is already registered`)
    endpoint.bound = true
    local remote = endpoint.remote
    if definition.kind == "request" then
        assert(remote:IsA("RemoteEvent"), "RoGrid requests require a reliable RemoteEvent")
        local returns = assert(definition.returns, "RoGrid request return types are missing")
        Transport.connect(remote, function(peer: Player?, ...: any)
            if not started then return end
            Requests.serve(id, remote, peer, handler, definition.arguments, returns, ...)
        end)
    else
        Transport.connect(remote, function(peer: Player?, ...: any)
            if not started then return end
            local message = validate(definition.arguments, ...)
            if message then
                if RunService:IsStudio() then warn(`RoGrid dropped {id}: {message}`) end
                return
            end
            local ok, failure
            if peer then
                ok, failure = xpcall(handler, debug.traceback, peer, ...)
            else
                ok, failure = xpcall(handler, debug.traceback, ...)
            end
            if not ok then warn(`RoGrid receiver {id} failed:\n{failure}`) end
        end)
    end
end

function Runtime.finish(): ()
    local folder = root
    assert(folder, "RoGrid startup has not begun")
    assert(not started, "RoGrid has already started")
    for id, endpoint in endpoints do
        assert(endpoint.definition.receiver ~= side or endpoint.bound, `RoGrid receiver {id} has not been registered`)
    end
    started = true
    Transport.publish(folder)
end

local function endpointFor(id: string, expected: string, destination: string, kind: string): Endpoint
    assert(started, "Call RoGrid.start before sending messages; do not call at receiver module top level")
    assert(expected == revision, "RoGrid callers are stale; regenerate and restart Play")
    local endpoint = endpoints[id]
    assert(endpoint, `Unknown RoGrid endpoint {id}`)
    assert(endpoint.definition.receiver == destination, "RoGrid endpoint has the wrong direction")
    assert(endpoint.definition.kind == kind, `RoGrid endpoint {id} is not a {kind}`)
    return endpoint
end

function Runtime.fireServer(id: string, expected: string, ...: any): ()
    assert(side == "client", "Server event callers can only be used on the client")
    Transport.send(endpointFor(id, expected, "server", "event").remote, nil, ...)
end

function Runtime.fireClient(id: string, expected: string, player: Player, ...: any): ()
    assert(side == "server", "Client event callers can only be used on the server")
    Transport.send(endpointFor(id, expected, "client", "event").remote, player, ...)
end

function Runtime.fireAllClients(id: string, expected: string, ...: any): ()
    assert(side == "server", "Client event callers can only be used on the server")
    Transport.broadcast(endpointFor(id, expected, "client", "event").remote, ...)
end

function Runtime.invokeServer(id: string, expected: string, timeoutSeconds: number?, ...: any): ...any
    assert(side == "client", "Server request callers can only be used on the client")
    local endpoint = endpointFor(id, expected, "server", "request")
    local remote = endpoint.remote
    assert(remote:IsA("RemoteEvent"), "RoGrid requests require a reliable RemoteEvent")
    return requests.invoke(id, remote, nil, assert(endpoint.definition.returns), timeoutSeconds, ...)
end

return Runtime
