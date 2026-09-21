local item = { { name = "item", shape = { record = { id = "string" } } } }
local payload = { { name = "payload", shape = { record = {
    heights = { array = "number" }, text = "string", bytes = "buffer",
} } } }

return {
    { id = "server.Tests.send", receiver = "server", kind = "event", reliability = "reliable", arguments = {
        item[1], { name = "note", shape = { optional = "string" } },
    } },
    { id = "client.Tests.reply", receiver = "client", kind = "event", reliability = "reliable", arguments = item },
    { id = "server.Tests.large", receiver = "server", kind = "event", reliability = "reliable", arguments = payload },
    { id = "client.Tests.largeReply", receiver = "client", kind = "event", reliability = "reliable", arguments = payload },
}
