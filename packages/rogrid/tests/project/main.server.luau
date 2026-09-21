local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local ServerScriptService = game:GetService("ServerScriptService")
local StudioTestService = game:GetService("StudioTestService")
local RoGrid = require(ReplicatedStorage.Packages.rogrid)
local Client = require(ServerScriptService.RoGridGenerated.Client)
RoGrid.start()
assert(not pcall(RoGrid.start), "starting twice must fail")
local function waitFor(description, ready)
    local deadline = os.clock() + 45
    while not ready() do
        if os.clock() >= deadline then StudioTestService:EndTest("FAIL: timeout: " .. description); return end
        task.wait()
    end
end
waitFor("both clients ready", function()
    local players = Players:GetPlayers()
    return #players == 2 and players[1]:GetAttribute("TestReady") and players[2]:GetAttribute("TestReady")
end)
-- Bypass the typed caller to simulate invalid input arriving at the client.
ReplicatedStorage.RoGridRemotes["client.Notifications.targeted"]:FireAllClients("wrong", 123)
Client.Notifications.broadcast.fireAll("everyone")
assert(ReplicatedStorage.RoGridRemotes["server.Lobby.pose"]:IsA("UnreliableRemoteEvent"))
assert(ReplicatedStorage.RoGridRemotes["client.Notifications.pose"]:IsA("UnreliableRemoteEvent"))
task.spawn(function()
    for sequence = 1, 20 do
        Client.Notifications.pose.fireAll(sequence, Vector3.new(1, 2, 3))
        task.wait(0.1)
    end
end)
waitFor("both clients verified delivery", function()
    local players = Players:GetPlayers()
    return #players == 2 and players[1]:GetAttribute("TestDone") and players[2]:GetAttribute("TestDone")
end)
StudioTestService:EndTest("PASS: generated startup, two-client targeting, broadcast, typed requests, optional timeouts, unreliable events, sender identity and handler recovery")
