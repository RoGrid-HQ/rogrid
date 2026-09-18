--!strict
local Server = require(game:GetService("ReplicatedStorage").RoGridGenerated.Server)

return function(): ()
    Server.Lobby.setReady.fire(true)
end
