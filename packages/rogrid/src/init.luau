--!strict
local client = require("@self/client")
local protocol = require("@self/protocol")
local request = require("@self/request")
local server = require("@self/server")

export type Guard = request.Guard

local RoGrid = {}

RoGrid.version = "0.2.0"

-- Client to server. The client waits for the answer.
RoGrid.request = request.new

-- Opens the boundary. Server only.
RoGrid.start = server.start

-- For generated code. Not part of the public API.
RoGrid.runtime = table.freeze({
	version = protocol.runtime,
	caller = client.caller,
})

return RoGrid
