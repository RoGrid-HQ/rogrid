--!strict
-- Constants shared by the server, the client, and generated code.

return table.freeze({
	-- Bumped whenever generated code and this library stop being compatible.
	runtime = 1,
	-- The RemoteFunction that RoGrid.start creates in ReplicatedStorage.
	requestRemote = "RoGridRequest",
})
