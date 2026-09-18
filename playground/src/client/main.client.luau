--!strict
local ReplicatedStorage = game:GetService("ReplicatedStorage")

local Packages = ReplicatedStorage:WaitForChild("Packages")
local RoGrid = require(Packages:WaitForChild("rogrid"))

RoGrid.start()

-- Optional example game behavior, after RoGrid is ready.
require(script.Parent:WaitForChild("ReadyDemo"))()
