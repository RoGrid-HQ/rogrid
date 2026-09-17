--!strict
-- What happens to one request once it reaches the server. There are no Roblox
-- APIs in this file, so it behaves the same under any transport and in tests.

export type Definition = {
	handler: (...any) -> ...any,
	guards: { (player: any, input: any) -> (boolean, string?) },
}

export type Entry = {
	definition: Definition,
	-- Describes what is wrong with the input, or returns nil.
	-- Absent when the request takes no input.
	check: ((input: any) -> string?)?,
}

export type Registry = { [string]: Entry }

local pipeline = {}

-- Returns `true, value` or `false, code, detail`.
-- The code is safe to send to the client. The detail is for the server log only.
function pipeline.run(registry: Registry, player: any, name: any, input: any): (boolean, any, string?)
	local entry = if type(name) == "string" then registry[name] else nil
	if not entry then
		return false, "unknown-function", `no request named {tostring(name)}`
	end

	if entry.check then
		local problem = entry.check(input)
		if problem then
			return false, "invalid-input", `{name}: input{problem}`
		end
	elseif input ~= nil then
		return false, "invalid-input", `{name}: takes no input`
	end

	-- Guards and the handler are user code, so both run protected.
	local finished, passed, value = xpcall(function(): (boolean, any)
		for _, guard in entry.definition.guards do
			local allowed, reason = guard(player, input)
			if not allowed then
				return false, if type(reason) == "string" then reason else "rejected"
			end
		end
		return true, (entry.definition.handler(player, input))
	end, debug.traceback)

	if not finished then
		-- `passed` holds the traceback here.
		return false, "internal", `{name}: {passed}`
	end
	if not passed then
		-- A guard refused. `value` holds its reason.
		return false, value
	end
	return true, value
end

return pipeline
