local ReplicatedStorage = game:GetService("ReplicatedStorage")
local RoGrid = require(ReplicatedStorage:WaitForChild("RoGrid"))
local validate = require(ReplicatedStorage.RoGrid.validate)
local probe = ReplicatedStorage:WaitForChild("Probe")
local values = require(ReplicatedStorage:WaitForChild("Native"))(getfenv())
values.Instance = ReplicatedStorage:WaitForChild("SharedModel")
local expected, received = 0, 0
for _ in values do expected += 1 end
local runtimeReplies = 0
local largeReplies = 0
local function finish()
    if received == expected and runtimeReplies == 100 and largeReplies == 1 then probe:FireServer("done", received) end
end
local runtime = RoGrid._runtime
local waitWarning = false
local warningConnection = game:GetService("LogService").MessageOut:Connect(function(message, kind)
    if kind == Enum.MessageType.MessageWarning and string.find(message, "RoGridEvents", 1, true) then
        waitWarning = true
    end
end)
local beganWaiting = os.clock()
probe:FireServer("waiting")
runtime.begin("client", "test", { "server.Tests.send", "client.Tests.reply", "server.Tests.large", "client.Tests.largeReply" })
warningConnection:Disconnect()
if not waitWarning or os.clock() - beganWaiting < 10 then
    probe:FireServer("failure", "expected an engine warning and successful startup after the former deadline")
end
runtime.bind("client.Tests.reply", function(item)
    assert(item.id == "ok")
    runtimeReplies += 1
    finish()
end, { { name = "item", shape = { record = { id = "string" } } } })
runtime.bind("client.Tests.largeReply", function(payload)
    assert(#payload.heights == 3000 and payload.heights[3000] == 42 and #payload.text == 8192)
    assert(buffer.len(payload.bytes) == 131072 and buffer.readu8(payload.bytes, 131071) == 7)
    largeReplies += 1
    finish()
end, { { name = "payload", shape = { record = { heights = { array = "number" }, text = "string", bytes = "buffer" } } } })
runtime.finish()
probe.OnClientEvent:Connect(function(name, value)
    local shape = if name == "Instance" then { instance = "Model" } else name
    local message = validate({ { name = name, shape = shape } }, value)
    if message then probe:FireServer("failure", "server to client: " .. message) end
    received += 1
    finish()
end)
runtime.fireServer("server.Tests.send", "test", { id = 1 })
runtime.fireServer("server.Tests.send", "test", { id = "bad", [string.rep("a", 63) .. "é_tail"] = true })
runtime.fireServer("server.Tests.send", "test")
runtime.fireServer("server.Tests.send", "test", { id = "bad" }, nil, nil)
-- A burst above the former 60-message limit must reach the handler in full.
for _ = 1, 100 do
    runtime.fireServer("server.Tests.send", "test", { id = "ok" })
end
local payload = { heights = table.create(3000, 42), text = string.rep("a", 8192), bytes = buffer.create(131072) }
buffer.writeu8(payload.bytes, 131071, 7)
runtime.fireServer("server.Tests.large", "test", payload)
for name, value in values do
    local ok, message = pcall(function() probe:FireServer(name, value) end)
    if not ok then
        probe:FireServer("failure", name .. ": " .. tostring(message))
        received += 1
    end
end
finish()
