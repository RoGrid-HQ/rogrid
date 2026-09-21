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
local largeHandled = 0
local dropped = 0
local completeFieldName = false
game:GetService("LogService").MessageOut:Connect(function(message, kind)
    if kind == Enum.MessageType.MessageWarning and string.find(message, "RoGrid dropped server.Tests.send:", 1, true) then
        dropped += 1
        if message == "RoGrid dropped server.Tests.send: item." .. string.rep("a", 63) .. "é_tail: unknown field" then
            completeFieldName = true
        end
    end
end)
local runtime = RoGrid._runtime
runtime.begin("server", "test", require(ReplicatedStorage.Definitions))
runtime.bind("server.Tests.send", function(player, item, note)
    handled += 1
    if item.id ~= "ok" or note ~= nil or not player:IsA("Player") then
        table.insert(failures, "invalid payload reached handler")
        return
    end
    runtime.fireClient("client.Tests.reply", "test", player, item)
end)
runtime.bind("server.Tests.large", function(player, payload)
    largeHandled += 1
    if #payload.heights ~= 3000 or payload.heights[3000] ~= 42 or #payload.text ~= 8192
        or buffer.len(payload.bytes) ~= 131072 or buffer.readu8(payload.bytes, 131071) ~= 7 then
        table.insert(failures, "large payload changed in transit")
    end
    runtime.fireClient("client.Tests.largeReply", "test", player, payload)
end)
probe.OnServerEvent:Connect(function(player, name, value)
    if name == "waiting" then
        -- Keep startup waiting long enough to exercise the engine warning.
        task.delay(12, runtime.finish)
        return
    end
    if name == "done" then
        if value ~= count or #failures > 0 or handled ~= 100 or largeHandled ~= 1 or dropped ~= 4 or not completeFieldName then
            StudioTestService:EndTest(`FAIL: handled {handled}/100 messages and {largeHandled}/1 large payload, reported {dropped}/4 invalid payloads, complete field name: {completeFieldName}; ` .. table.concat(failures, "; "))
        else
            StudioTestService:EndTest("PASS: delayed startup with engine warning, " .. count .. " native round trips, 100-message runtime burst, large payload round trip and every invalid payload reported with full field names")
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
    StudioTestService:EndTest(`FAIL: timed out; handled {handled}/100 messages and {largeHandled}/1 large payload; ` .. table.concat(failures, "; "))
end)
