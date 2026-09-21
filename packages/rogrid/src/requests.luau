--!strict
local HttpService = game:GetService("HttpService")
local Players = game:GetService("Players")
local RunService = game:GetService("RunService")
local Transport = require(script.Parent.transport)
local validate = require(script.Parent.validate)

export type ErrorCode = "Timeout" | "InvalidRequest" | "InvalidResponse" | "HandlerError" | "SendFailed" | "Disconnected"
type ErrorFields = { code: ErrorCode, endpoint: string }
local errorMetatable = {
    __tostring = function(value: ErrorFields): string
        return `RoGrid request {value.endpoint} failed: {value.code}`
    end,
}
export type RequestError = typeof(setmetatable({} :: ErrorFields, errorMetatable))
type Values = { [number]: any, n: number }
type Outcome = { ok: true, values: Values } | { ok: false, failure: RequestError }
type Pending = {
    endpoint: string,
    remote: RemoteEvent,
    peer: Player?,
    returns: { validate.Parameter },
    thread: thread,
    waiting: boolean,
    timer: thread?,
    deadline: number?,
    outcome: Outcome?,
}

local Requests = {}

local function failure(endpoint: string, code: ErrorCode): RequestError
    return setmetatable({ code = code, endpoint = endpoint } :: ErrorFields, errorMetatable)
end

local function invalid(endpoint: string, message: string): ()
    if RunService:IsStudio() then
        warn(`RoGrid dropped {endpoint}: {message}`)
    end
end

function Requests.new()
    local pending: { [string]: Pending } = {}
    local manager = {}

    local function settle(id: string, entry: Pending, outcome: Outcome): ()
        if pending[id] ~= entry then return end
        pending[id] = nil
        entry.outcome = outcome
        if entry.timer then
            task.cancel(entry.timer)
            entry.timer = nil
        end
        if entry.waiting and coroutine.status(entry.thread) == "suspended" then
            task.defer(entry.thread)
        end
    end

    function manager.receive(remote: RemoteEvent, peer: Player?, id: any, status: any, ...: any): ()
        if typeof(id) ~= "string" then return end
        local entry = pending[id]
        if not entry or entry.remote ~= remote or entry.peer ~= peer then return end
        local deadline = entry.deadline
        if deadline and os.clock() >= deadline then
            settle(id, entry, { ok = false, failure = failure(entry.endpoint, "Timeout") })
            return
        end
        if status == "ok" then
            local message = validate(entry.returns, ...)
            if message then
                invalid(entry.endpoint, message)
                settle(id, entry, { ok = false, failure = failure(entry.endpoint, "InvalidResponse") })
            else
                settle(id, entry, { ok = true, values = table.pack(...) })
            end
        else
            local code = ...
            if status ~= "error" or select("#", ...) ~= 1
                or (code ~= "InvalidRequest" and code ~= "InvalidResponse" and code ~= "HandlerError") then
                code = "InvalidResponse"
            end
            settle(id, entry, { ok = false, failure = failure(entry.endpoint, code) })
        end
    end

    function manager.invoke(endpoint: string, remote: RemoteEvent, peer: Player?, returns: { validate.Parameter }, timeoutSeconds: number?, ...: any): ...any
        assert(coroutine.isyieldable(), "RoGrid requests require a yieldable thread")
        assert(timeoutSeconds == nil or (typeof(timeoutSeconds) == "number" and timeoutSeconds >= 0 and timeoutSeconds < math.huge),
            "request timeout must be a finite, non-negative number or nil")
        if timeoutSeconds == 0 then error(failure(endpoint, "Timeout"), 0) end
        local id = HttpService:GenerateGUID(false)
        local entry: Pending = {
            endpoint = endpoint, remote = remote, peer = peer, returns = returns,
            thread = coroutine.running(), waiting = false,
            deadline = if timeoutSeconds then os.clock() + timeoutSeconds else nil,
        }
        if timeoutSeconds then
            entry.timer = task.delay(timeoutSeconds, function()
                entry.timer = nil
                settle(id, entry, { ok = false, failure = failure(endpoint, "Timeout") })
            end)
        end
        pending[id] = entry
        local sent = pcall(Transport.send, remote, peer, id, ...)
        if not sent then
            settle(id, entry, { ok = false, failure = failure(endpoint, "SendFailed") })
        end
        if not entry.outcome then
            entry.waiting = true
            coroutine.yield()
        end
        local outcome = entry.outcome
        assert(outcome, "RoGrid request resumed without a response")
        if not outcome.ok then error(outcome.failure, 0) end
        local values = outcome.values
        return table.unpack(values, 1, values.n)
    end

    function manager.disconnect(peer: Player?): ()
        for id, entry in pending do
            if entry.peer == peer then
                settle(id, entry, { ok = false, failure = failure(entry.endpoint, "Disconnected") })
            end
        end
    end

    return manager
end

function Requests.serve(endpoint: string, remote: RemoteEvent, peer: Player?, handler: (...any) -> ...any, arguments: { validate.Parameter }, returns: { validate.Parameter }, id: any, ...: any): ()
    if typeof(id) ~= "string" then return end
    local message = validate(arguments, ...)
    local status: string
    local values: Values
    if message then
        invalid(endpoint, message)
        status, values = "error", table.pack("InvalidRequest")
    else
        local result: Values
        if peer then
            result = table.pack(xpcall(handler, debug.traceback, peer, ...))
        else
            result = table.pack(xpcall(handler, debug.traceback, ...))
        end
        if not result[1] then
            warn(`RoGrid receiver {endpoint} failed:\n{result[2]}`)
            status, values = "error", table.pack("HandlerError")
        else
            local resultMessage = validate(returns, table.unpack(result, 2, result.n))
            if resultMessage then
                warn(`RoGrid receiver {endpoint} returned an invalid response: {resultMessage}`)
                status, values = "error", table.pack("InvalidResponse")
            else
                status, values = "ok", table.pack(table.unpack(result, 2, result.n))
            end
        end
    end
    if peer and peer.Parent ~= Players then return end
    local sent, sendError = pcall(Transport.send :: (...any) -> ...any, remote, peer, id, status, table.unpack(values, 1, values.n))
    if not sent then warn(`RoGrid response {endpoint} could not be sent: {sendError}`) end
end

return Requests
