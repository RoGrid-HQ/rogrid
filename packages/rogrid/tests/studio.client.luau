local ReplicatedStorage = game:GetService("ReplicatedStorage")
local RoGrid = require(ReplicatedStorage:WaitForChild("RoGrid"))
local validate = require(ReplicatedStorage.RoGrid.validate)
local probe = ReplicatedStorage:WaitForChild("Probe")
local values = require(ReplicatedStorage:WaitForChild("Native"))(getfenv())
values.Instance = ReplicatedStorage:WaitForChild("SharedModel")
local expected, received = 0, 0
for _ in values do expected += 1 end
local runtimeReplies = 0
local function finish()
    if received == expected and runtimeReplies == 100 then probe:FireServer("done", received) end
end
local runtime = RoGrid._runtime
runtime.begin("client", "test", { "server.Tests.send", "client.Tests.reply" })
runtime.bind("client.Tests.reply", function(item)
    assert(item.id == "ok")
    runtimeReplies += 1
    finish()
end, { { name = "item", shape = { record = { id = "string" } } } })
runtime.finish()
probe.OnClientEvent:Connect(function(name, value)
    local shape = if name == "Instance" then { instance = "Model" } else name
    local message = validate({ { name = name, shape = shape } }, value)
    if message then probe:FireServer("failure", "server to client: " .. message) end
    received += 1
    finish()
end)
runtime.fireServer("server.Tests.send", "test", { id = 1 })
runtime.fireServer("server.Tests.send", "test", { id = "bad", unknown = true })
runtime.fireServer("server.Tests.send", "test")
runtime.fireServer("server.Tests.send", "test", { id = "bad" }, nil, nil)
-- A burst above the former 60-message limit must reach the handler in full.
for _ = 1, 100 do
    runtime.fireServer("server.Tests.send", "test", { id = "ok" })
end
for name, value in values do
    local ok, message = pcall(function() probe:FireServer(name, value) end)
    if not ok then
        probe:FireServer("failure", name .. ": " .. tostring(message))
        received += 1
    end
end
finish()
