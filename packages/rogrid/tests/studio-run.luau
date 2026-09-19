-- Run at plugin security using run-in-roblox, after building studio.project.json.
-- Studio copies local plugins into test DataModels. Only the edit copy owns the run.
if not game:GetService("RunService"):IsEdit() then task.wait(120); return end
local result = game:GetService("StudioTestService"):ExecutePlayModeAsync({})
assert(typeof(result) == "string" and string.sub(result, 1, 5) == "PASS:", tostring(result))
print(result)
