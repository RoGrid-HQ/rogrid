--!strict
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local ServerScriptService = game:GetService("ServerScriptService")
local RoGrid = require(ReplicatedStorage.Packages.rogrid)
local Client = require(ServerScriptService.RoGridGenerated.Client)

return {
    setReady = RoGrid.event(function(player: Player, ready: boolean)
        if player:GetAttribute("Ready") == ready then
            return
        end
        player:SetAttribute("Ready", ready)
        local status = if ready then "ready" else "not ready"
        Client.Notifications.show.fireAll(`{player.DisplayName} is {status}.`)
    end),
}
