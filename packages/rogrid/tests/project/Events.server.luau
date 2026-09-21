--!strict
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local ServerScriptService = game:GetService("ServerScriptService")
local RoGrid = require(ReplicatedStorage.Packages.rogrid)
local Client = require(ServerScriptService.RoGridGenerated.Client)

return {
    inspect = RoGrid.request(function(player: Player, value: {id: string, position: Vector3?}, delaySeconds: number?, note: string?): (Player, {id: string, position: Vector3?}, string?)
        if delaySeconds then task.wait(delaySeconds) end
        return player, value, note
    end),
    acknowledge = RoGrid.request(function(_player: Player): () end),
    failRequest = RoGrid.request(function(_player: Player): ()
        error("intentional request failure")
    end),
    badReturn = RoGrid.request(function(_player: Player): number
        return ("wrong" :: any)
    end),
    pose = RoGrid.event(function(player: Player, sequence: number, position: Vector3)
        assert(position == Vector3.new(1, 2, 3))
        player:SetAttribute("ServerPose", sequence)
    end, { reliability = "unreliable" }),
    ready = RoGrid.event(function(player: Player, claimedPlayer: Player)
        assert(player == claimedPlayer, "sender identity changed")
        player:SetAttribute("TestReady", true)
    end),
    echo = RoGrid.event(function(player: Player, value: {id: string, position: Vector3?}, note: string?)
        assert(note == nil and value.id == "hello")
        Client.Notifications.targeted.fire(player, player, value.id)
    end),
    fail = RoGrid.event(function(_player: Player)
        error("intentional receiver failure")
    end),
    done = RoGrid.event(function(player: Player, targeted: number, broadcast: number)
        assert(targeted == 1 and broadcast == 1, "incorrect delivery count")
        player:SetAttribute("TestDone", true)
    end),
}
