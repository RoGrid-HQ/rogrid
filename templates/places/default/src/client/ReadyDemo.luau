--!strict
local Server = require(game:GetService("ReplicatedStorage").RoGridGenerated.Server)

return function(): ()
    local ok, ready = pcall(Server.Lobby.getReady.invoke, 5)
    if ok then
        print(`Current ready status: {ready}`)
    else
        warn(`Could not read ready status: {ready}`)
    end
    Server.Lobby.setReady.fire(true)
end
