local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local RoGrid = require(ReplicatedStorage.Packages.rogrid)
local Server = require(ReplicatedStorage.RoGridGenerated.Server)
RoGrid.start()
assert(not pcall(RoGrid.start), "starting twice must fail")
Server.Lobby.ready.fire(Players.LocalPlayer)
local value = { id = "hello", position = Vector3.new(1, 2, 3) }
local sender, echoed, note = Server.Lobby.inspect.invoke(value, nil, nil, 10)
assert(sender == Players.LocalPlayer and echoed.id == value.id and echoed.position == value.position and note == nil)
assert(select("#", Server.Lobby.acknowledge.invoke(10)) == 0)
local noDeadline = table.pack(Server.Lobby.inspect.invoke(value))
assert(noDeadline.n == 3 and noDeadline[1] == Players.LocalPlayer and noDeadline[3] == nil)
for _, case in {
    { Server.Lobby.failRequest.invoke, "HandlerError" },
    { Server.Lobby.badReturn.invoke, "InvalidResponse" },
} do
    local ok, failure = pcall(case[1], 10)
    assert(not ok and failure.code == case[2])
end
local ok, failure = pcall(Server.Lobby.inspect.invoke, value, 0.5, nil, 0.05)
assert(not ok and failure.code == "Timeout")
local completed = 0
for i = 1, 10 do
    task.spawn(function()
        local actual, response = Server.Lobby.inspect.invoke({ id = tostring(i) }, (11 - i) * 0.01, nil, 10)
        assert(actual == Players.LocalPlayer and response.id == tostring(i))
        completed += 1
    end)
end
-- Repeat transient samples without requiring full or ordered delivery.
task.spawn(function()
    for sequence = 1, 20 do
        Server.Lobby.pose.fire(sequence, Vector3.new(1, 2, 3))
        task.wait(0.1)
    end
end)
Server.Lobby.fail.fire()
Server.Lobby.echo.fire({ id = "hello", position = Vector3.new(1, 2, 3) })
local deadline = os.clock() + 40
while Players.LocalPlayer:GetAttribute("Targeted") ~= 1 or Players.LocalPlayer:GetAttribute("Broadcast") ~= 1
    or Players.LocalPlayer:GetAttribute("ClientPose") == nil or Players.LocalPlayer:GetAttribute("ServerPose") == nil
    or completed ~= 10 do
    assert(os.clock() < deadline, "client delivery timed out")
    task.wait()
end
-- Allow accidental broadcasts/duplicates queued for the other player to surface.
task.wait(0.5)
Server.Lobby.done.fire(Players.LocalPlayer:GetAttribute("Targeted"), Players.LocalPlayer:GetAttribute("Broadcast"))
