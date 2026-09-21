--!strict
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local RunService = game:GetService("RunService")

export type Remote = RemoteEvent | UnreliableRemoteEvent
export type Receiver = (Player?, ...any) -> ()

local Transport = {}

function Transport.open(revision: string): Folder
    if RunService:IsServer() then
        assert(not ReplicatedStorage:FindFirstChild("RoGridRemotes"), "RoGridRemotes already exists")
        local folder = Instance.new("Folder")
        folder.Name = "RoGridRemotes"
        folder:SetAttribute("Revision", revision)
        return folder
    end
    local folder = ReplicatedStorage:WaitForChild("RoGridRemotes")
    assert(folder:IsA("Folder"), "RoGridRemotes must be a Folder")
    assert(folder:GetAttribute("Revision") == revision, "RoGrid client/server generation differs; restart Play after running rogrid dev")
    return folder
end

function Transport.remote(folder: Folder, id: string, unreliable: boolean): Remote
    local className = if unreliable then "UnreliableRemoteEvent" else "RemoteEvent"
    if RunService:IsServer() then
        local remote: Instance = if unreliable then Instance.new("UnreliableRemoteEvent") else Instance.new("RemoteEvent")
        remote.Name = id
        remote.Parent = folder
        return remote :: Remote
    end
    local remote = folder:WaitForChild(id)
    assert(remote.ClassName == className, `RoGrid endpoint {id} must be a {className}`)
    return remote :: Remote
end

function Transport.publish(folder: Folder): ()
    if RunService:IsServer() then
        folder.Parent = ReplicatedStorage
    end
end

function Transport.connect(remote: Remote, receiver: Receiver): RBXScriptConnection
    -- Narrow each class before calling its nominally typed engine API.
    if RunService:IsServer() then
        if remote:IsA("RemoteEvent") then
            return remote.OnServerEvent:Connect(receiver)
        end
        return remote.OnServerEvent:Connect(receiver)
    end
    local function receive(...: any)
        receiver(nil, ...)
    end
    if remote:IsA("RemoteEvent") then
        return remote.OnClientEvent:Connect(receive)
    end
    return remote.OnClientEvent:Connect(receive)
end

-- A nil peer addresses the server. A Player addresses that client.
-- Endpoint direction is checked by the caller; replies use the same transport.
function Transport.send(remote: Remote, peer: Player?, ...: any): ()
    if peer then
        if remote:IsA("RemoteEvent") then remote:FireClient(peer, ...)
        else remote:FireClient(peer, ...) end
    else
        if remote:IsA("RemoteEvent") then remote:FireServer(...)
        else remote:FireServer(...) end
    end
end

function Transport.broadcast(remote: Remote, ...: any): ()
    if remote:IsA("RemoteEvent") then remote:FireAllClients(...)
    else remote:FireAllClients(...) end
end

return Transport
