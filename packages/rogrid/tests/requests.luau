local fixture = require("./support/runtime")
local count = 0
local id = "server.Tests.read"
local function test(name, run)
    local ok, message = pcall(run)
    assert(ok, `{name}: {message}`)
    count += 1
end
local function setup(server, handler, returns)
    local f = fixture(server, true)
    local definitions = {
        { id = id, receiver = "server", kind = "request",
            arguments = { { name = "value", shape = "number" }, { name = "note", shape = { optional = "string" } } },
            returns = returns or { { name = "returns[1]", shape = "number" }, { name = "returns[2]", shape = { optional = "string" } } } },
        { id = "server.Tests.other", receiver = "server", kind = "request", arguments = {}, returns = {} },
    }
    if not server then f.published("test", definitions) end
    f.begin(if server then "server" else "client", "test", definitions)
    if server then f.bind(id, handler or function(_, value, note) return value, note end) end
    f.finish()
    return f
end
local function call(f, timeout, ...)
    local args = table.pack(...)
    local result
    local thread = f.spawn(function()
        result = table.pack(pcall(f.runtime.invokeServer, id, "test", timeout, table.unpack(args, 1, args.n)))
    end)
    return function() return result end, thread
end
local function respond(f, sent, ...)
    sent.remote.OnClientEvent:fire(sent.args[1], ...)
    f.advance(0)
end
local function failed(result, code)
    assert(result and result[1] == false and result[2].code == code and result[2].endpoint == id)
    assert(string.find(tostring(result[2]), code, 1, true))
end

test("concurrent replies can finish in any order and preserve trailing nil", function()
    local f = setup(false)
    local results = {}
    for i = 1, 200 do results[i] = call(f, nil, i, nil) end
    assert(f.timers() == 0)
    for i = 200, 1, -1 do respond(f, f.sent[i], "ok", i, nil) end
    for i, result in results do assert(result().n == 3 and result()[1] and result()[2] == i and result()[3] == nil) end
end)

test("reply arriving during send is available before the caller yields", function()
    local f = setup(false)
    f.onSend = function(sent) sent.remote.OnClientEvent:fire(sent.args[1], "ok", 42, nil) end
    local result, thread = call(f, 5, 42)
    assert(coroutine.status(thread) == "dead" and result()[2] == 42 and result().n == 3)
    assert(f.timers() == 0)
end)

test("omitting timeout keeps waiting as time advances", function()
    local f = setup(false)
    local result, thread = call(f, nil, 7)
    f.advance(86400)
    assert(not result() and coroutine.status(thread) == "suspended" and f.timers() == 0)
    respond(f, f.sent[1], "ok", 7)
    assert(result()[2] == 7)
end)

test("timeouts settle once and discard late replies", function()
    local f = setup(false)
    local result, thread = call(f, 5, 7)
    f.advance(4)
    assert(not result())
    f.advance(1)
    failed(result(), "Timeout")
    assert(coroutine.status(thread) == "dead" and f.timers() == 0)
    respond(f, f.sent[1], "ok", 7)
    failed(result(), "Timeout")
end)

test("the deadline also applies when a reply arrives before the timer is resumed", function()
    local f = setup(false)
    local result = call(f, 5, 7)
    f.now = 5
    respond(f, f.sent[1], "ok", 7)
    failed(result(), "Timeout")
    assert(f.timers() == 0)
end)

test("success cancels the timer and duplicate or unknown replies are ignored", function()
    local f = setup(false)
    local result = call(f, 5, 7)
    f.remote(id).OnClientEvent:fire("unknown", "ok", 99)
    assert(not result())
    respond(f, f.sent[1], "ok", 7)
    respond(f, f.sent[1], "ok", 99)
    f.advance(10)
    assert(result()[2] == 7 and f.timers() == 0)
end)

test("malformed responses and response envelopes fail the matching request", function()
    for _, response in {
        { "ok", "wrong" }, { "ok", 1, nil, true }, { "error", "private traceback" },
        { "error", "HandlerError", "extra" }, { true, 1 },
    } do
        local f = setup(false)
        local result = call(f, 5, 1)
        respond(f, f.sent[1], table.unpack(response, 1, if response[4] then 4 else #response))
        failed(result(), "InvalidResponse")
        assert(f.timers() == 0)
    end
    for _, code in { "InvalidRequest", "InvalidResponse", "HandlerError" } do
        local f = setup(false)
        local result = call(f, nil, 1)
        respond(f, f.sent[1], "error", code)
        failed(result(), code)
    end
end)

test("send failures and player departure release pending calls", function()
    local f = setup(false)
    f.onSend = function() error("send failed") end
    local result = call(f, 5, 1)
    failed(result(), "SendFailed")
    assert(f.timers() == 0)
    f = setup(false)
    result = call(f, 5, 1)
    f.services.Players.PlayerRemoving:fire(f.player)
    f.advance(0)
    failed(result(), "Disconnected")
    assert(f.timers() == 0)
end)

test("timeouts are checked before sending", function()
    for _, timeout in { -1, math.huge, 0 / 0, "five" } do
        local f = setup(false)
        local result = call(f, timeout, 1)
        assert(not result()[1] and string.find(tostring(result()[2]), "request timeout", 1, true))
        assert(#f.sent == 0 and f.timers() == 0)
    end
    local f = setup(false)
    local result = call(f, 0, 1)
    failed(result(), "Timeout")
    assert(#f.sent == 0)
end)

test("server validates arguments and results and returns fixed failure codes", function()
    local calls = 0
    local f = setup(true, function(player, value, note)
        assert(player:IsA("Player"))
        calls += 1
        if value == 1 then error("private handler detail") end
        if value == 2 then return "wrong" end
        return value, note
    end)
    local remote = f.remote(id)
    remote.OnServerEvent:fire(f.player, {}, 3)
    assert(calls == 0 and #f.sent == 0)
    remote.OnServerEvent:fire(f.player, "a", "wrong")
    assert(calls == 0 and f.sent[1].args[3] == "error" and f.sent[1].args[4] == "InvalidRequest")
    remote.OnServerEvent:fire(f.player, "b", 1)
    remote.OnServerEvent:fire(f.player, "c", 2)
    remote.OnServerEvent:fire(f.player, "d", 42, nil)
    assert(calls == 3)
    assert(f.sent[2].args[4] == "HandlerError" and f.sent[2].args.n == 4)
    assert(f.sent[3].args[4] == "InvalidResponse")
    assert(f.sent[4].args[1] == f.player and f.sent[4].args[2] == "d" and f.sent[4].args[3] == "ok")
    assert(f.sent[4].args[4] == 42 and f.sent[4].args.n == 5)
    assert(#f.warnings == 3 and string.find(f.warnings[2], "private handler detail", 1, true))
end)

test("acknowledgements send and receive zero result values", function()
    local server = setup(true, function() end, {})
    server.remote(id).OnServerEvent:fire(server.player, "a", 1)
    assert(server.sent[1].args.n == 3 and server.sent[1].args[3] == "ok")
    local client = setup(false, nil, {})
    local result = call(client, nil, 1)
    respond(client, client.sent[1], "ok")
    assert(result().n == 1 and result()[1])
end)

test("timing out a caller does not stop its receiver handler", function()
    local client = setup(false)
    local server
    local completed = false
    server = setup(true, function(_, value)
        server.task.wait(10)
        completed = true
        return value, nil
    end)
    client.onSend = function(sent)
        server.spawn(function() server.remote(id).OnServerEvent:fire(server.player, table.unpack(sent.args, 1, sent.args.n)) end)
    end
    server.onSend = function(sent)
        client.remote(id).OnClientEvent:fire(table.unpack(sent.args, 2, sent.args.n))
    end
    local result = call(client, 5, 42)
    client.advance(5)
    failed(result(), "Timeout")
    assert(not completed)
    server.advance(10)
    client.advance(0)
    assert(completed)
    failed(result(), "Timeout")
end)

test("reply matching includes both the remote and the expected player", function()
    local f = setup(true)
    local manager = f.require(f.package.requests).new()
    local remote = f.remote(id)
    local other = f.node("Player", "Other", f.services.Players)
    local result
    f.spawn(function() result = table.pack(manager.invoke(id, remote, f.player, { { name = "result", shape = "number" } }, nil, 1)) end)
    local requestId = f.sent[1].args[2]
    manager.receive(remote, other, requestId, "ok", 1)
    manager.receive(f.remote("server.Tests.other"), f.player, requestId, "ok", 1)
    assert(not result)
    manager.receive(remote, f.player, requestId, "ok", 42)
    f.advance(0)
    assert(result[1] == 42)
end)

test("unreliable events share validation and all native sending operations", function()
    for _, server in { true, false } do
        local f = fixture(server, true)
        local definitions = f.events({ "server.Tests.pose", "client.Tests.pose" }, { { name = "value", shape = "number" } })
        for _, definition in definitions do definition.reliability = "unreliable" end
        if not server then f.published("test", definitions) end
        f.begin(if server then "server" else "client", "test", definitions)
        local incoming = if server then definitions[1].id else definitions[2].id
        local calls = 0
        f.bind(incoming, function() calls += 1 end)
        f.finish()
        assert(f.remote(incoming).ClassName == "UnreliableRemoteEvent")
        if server then
            f.runtime.fireClient(definitions[2].id, "test", f.player, 42)
            f.runtime.fireAllClients(definitions[2].id, "test", 7)
            assert(f.sent[1].method == "FireClient" and f.sent[2].method == "FireAllClients")
            f.remote(incoming).OnServerEvent:fire(f.player, "wrong")
            f.remote(incoming).OnServerEvent:fire(f.player, 1)
        else
            f.runtime.fireServer(definitions[1].id, "test", 42)
            assert(f.sent[1].method == "FireServer")
            f.remote(incoming).OnClientEvent:fire("wrong")
            f.remote(incoming).OnClientEvent:fire(1)
        end
        assert(calls == 1 and #f.warnings == 1)
    end
end)

print(`Requests and unreliable events: {count} scenarios passed`)
