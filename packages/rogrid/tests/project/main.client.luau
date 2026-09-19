local Players = game:GetService("Players")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local RoGrid = require(ReplicatedStorage.Packages.rogrid)
local Server = require(ReplicatedStorage.RoGridGenerated.Server)
RoGrid.start()
assert(not pcall(RoGrid.start), "starting twice must fail")
Server.Lobby.ready.fire(Players.LocalPlayer)
Server.Lobby.fail.fire()
Server.Lobby.echo.fire({ id = "hello", position = Vector3.new(1, 2, 3) })
local deadline = os.clock() + 40
while Players.LocalPlayer:GetAttribute("Targeted") ~= 1 or Players.LocalPlayer:GetAttribute("Broadcast") ~= 1 do
    assert(os.clock() < deadline, "client delivery timed out")
    task.wait()
end
-- Allow accidental broadcasts/duplicates queued for the other player to surface.
task.wait(0.5)
Server.Lobby.done.fire(Players.LocalPlayer:GetAttribute("Targeted"), Players.LocalPlayer:GetAttribute("Broadcast"))
