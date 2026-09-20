--!strict
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local Players = game:GetService("Players")
local RunService = game:GetService("RunService")
local ServerScriptService = game:GetService("ServerScriptService")

local RoGrid = {}

RoGrid.version = "0.2.2"

-- Internal contract with generated code, including when loaded through a package linker.
RoGrid._protocol = 3
local runtime = script:WaitForChild("runtime")
assert(runtime:IsA("ModuleScript"), "RoGrid runtime must be a ModuleScript; reinstall the package")
RoGrid._runtime = require(runtime)

-- Mark an explicitly typed receiver for the generator. No runtime reflection.
function RoGrid.event<A...>(handler: (A...) -> ()): (A...) -> ()
    return handler
end

-- The template calls this once before other game startup code.
function RoGrid.start(): ()
    local container: Instance
    if RunService:IsServer() then
        container = ServerScriptService
    else
        container = Players.LocalPlayer:WaitForChild("PlayerScripts")
        local shared = ReplicatedStorage:WaitForChild("RoGridGenerated")
        local callers = shared:WaitForChild("Server")
        assert(callers:IsA("ModuleScript"), "RoGrid callers must be a ModuleScript. Run rogrid dev.")
    end
    local generated = container:WaitForChild("RoGridGenerated")
    local startup = generated:WaitForChild("Start")
    assert(startup:IsA("ModuleScript"), "RoGrid startup must be a ModuleScript. Run rogrid dev.")
    local start = require(startup) :: () -> ()
    start()
end

return RoGrid
