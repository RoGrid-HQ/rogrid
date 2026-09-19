local ReplicatedStorage = game:GetService("ReplicatedStorage")
local StudioTestService = game:GetService("StudioTestService")
local RoGrid = require(ReplicatedStorage.RoGrid)
local validate = require(ReplicatedStorage.RoGrid.validate)
local probe = ReplicatedStorage.Probe
local values = require(ReplicatedStorage.Native)(getfenv())
values.Instance = ReplicatedStorage.SharedModel
local count = 0
for _ in values do count += 1 end
local failures = {}
local handled = 0
local runtime = RoGrid._runtime
runtime.begin("server", "test", { "server.Tests.send", "client.Tests.reply" })
runtime.bind("server.Tests.send", function(player, item, note)
    handled += 1
    if item.id ~= "ok" or note ~= nil or not player:IsA("Player") then
        table.insert(failures, "invalid payload reached handler")
        return
    end
    runtime.fireClient("client.Tests.reply", "test", player, item)
end, { { name = "item", shape = { record = { id = "string" } } }, { name = "note", shape = { optional = "string" } } })
runtime.finish()
probe.OnServerEvent:Connect(function(player, name, value)
    if name == "done" then
        if value ~= count or #failures > 0 or handled ~= 100 then
            StudioTestService:EndTest(`FAIL: handled {handled}/100 messages; ` .. table.concat(failures, "; "))
        else
            StudioTestService:EndTest("PASS: " .. count .. " native round trips and 100-message runtime burst")
        end
        return
    end
    if name == "failure" then table.insert(failures, value); return end
    if values[name] == nil then table.insert(failures, "unknown case: " .. tostring(name)); return end
    local shape = if name == "Instance" then { instance = "Model" } else name
    local message = validate({ { name = name, shape = shape } }, value)
    if message then table.insert(failures, "client to server: " .. message) end
    if name == "Instance" and value ~= values.Instance then table.insert(failures, "Instance identity changed") end
    probe:FireClient(player, name, values[name])
end)
task.delay(45, function()
    StudioTestService:EndTest(`FAIL: timed out; handled {handled}/100 messages; ` .. table.concat(failures, "; "))
end)
