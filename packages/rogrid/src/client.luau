--!strict
local protocol = require("./protocol")

local client = {}

local remote: RemoteFunction? = nil

-- Waits for the server to call RoGrid.start, so a call made early is not lost.
local function getRemote(): RemoteFunction
	if remote == nil then
		remote = game:GetService("ReplicatedStorage"):WaitForChild(protocol.requestRemote) :: RemoteFunction
	end
	return remote :: RemoteFunction
end

-- Used by the generated `api` module. Returns the function that calls one request.
function client.caller(name: string): (input: any) -> any
	return function(input: any): any
		local ok, result = getRemote():InvokeServer(name, input)
		if not ok then
			error(`RoGrid: {name} failed: {result}`, 2)
		end
		return result
	end
end

return client
