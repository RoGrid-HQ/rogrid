--!strict
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local RoGrid = require(ReplicatedStorage.Packages.rogrid)

return {
    show = RoGrid.event(function(message: string)
        print(message)
    end),
}
