--!strict
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local Players = game:GetService("Players")
local RunService = game:GetService("RunService")
local ServerScriptService = game:GetService("ServerScriptService")

local RoGrid = {}

RoGrid.version = "0.2.2"

-- Internal contract with generated code, including when loaded through a package linker.
RoGrid._protocol = 2
local runtime = script:WaitForChild("runtime", 10)
assert(runtime and runtime:IsA("ModuleScript"), "RoGrid runtime is missing; reinstall the package")
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
        local scripts = Players.LocalPlayer:WaitForChild("PlayerScripts", 10)
        assert(scripts, "PlayerScripts is missing")
        container = scripts
        local shared = ReplicatedStorage:WaitForChild("RoGridGenerated", 10)
        assert(shared, "RoGrid generated files are missing. Run rogrid dev before starting Play.")
        local callers = shared:WaitForChild("Server", 10)
        assert(callers and callers:IsA("ModuleScript"), "RoGrid callers are missing. Run rogrid dev.")
    end
    local generated = container:WaitForChild("RoGridGenerated", 10)
    assert(generated, "RoGrid generated files are missing. Run rogrid dev before starting Play.")
    local startup = generated:WaitForChild("Start", 10)
    assert(startup and startup:IsA("ModuleScript"), "RoGrid startup is missing. Run rogrid dev.")
    local start = require(startup) :: () -> ()
    start()
end

return RoGrid
