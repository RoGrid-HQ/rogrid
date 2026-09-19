--!strict
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local Players = game:GetService("Players")
local RoGrid = require(ReplicatedStorage.Packages.rogrid)

return {
    targeted = RoGrid.event(function(recipient: Player, message: string)
        assert(recipient == Players.LocalPlayer, "targeted event reached another player")
        assert(message == "hello", "invalid server payload reached the handler")
        Players.LocalPlayer:SetAttribute("Targeted", (Players.LocalPlayer:GetAttribute("Targeted") or 0) + 1)
    end),
    broadcast = RoGrid.event(function(message: string)
        assert(message == "everyone")
        Players.LocalPlayer:SetAttribute("Broadcast", (Players.LocalPlayer:GetAttribute("Broadcast") or 0) + 1)
    end),
}
