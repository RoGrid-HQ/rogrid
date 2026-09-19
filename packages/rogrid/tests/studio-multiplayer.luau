-- https://create.roblox.com/docs/reference/engine/classes/StudioTestService#ExecuteMultiplayerTestAsync
if not game:GetService("RunService"):IsEdit() then task.wait(120); return end
local result = game:GetService("StudioTestService"):ExecuteMultiplayerTestAsync(2, {})
assert(typeof(result) == "string" and string.sub(result, 1, 5) == "PASS:", tostring(result))
print(result)
