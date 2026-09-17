--!strict
-- RoGrid.request: something the client can call on the server and wait for.

-- Runs before the handler. Returns true, or false and a reason the client will see.
export type Guard = (player: Player, input: any) -> (boolean, string?)

export type Options = {
	guards: { Guard }?,
}

export type Request<F> = {
	kind: "request",
	handler: F,
	guards: { Guard },
}

type New = (<F>(handler: F) -> Request<F>) & (<F>(options: Options, handler: F) -> Request<F>)

local function new(first: any, second: any): any
	local options: Options = if second == nil then {} else first
	local handler = if second == nil then first else second

	if type(handler) ~= "function" then
		error("RoGrid.request expects a handler function", 2)
	end

	-- The editor catches this in strict mode. This catches it everywhere else.
	for key in options :: any do
		if key ~= "guards" then
			error(`RoGrid.request: unknown option "{key}"`, 2)
		end
	end

	return table.freeze({
		kind = "request",
		handler = handler,
		guards = options.guards or {},
	})
end

return {
	new = (new :: any) :: New,
}
