using System;
using System.Threading.Tasks;
using Microsoft.AspNetCore.SignalR;

namespace server_sample.Hubs;

public class EchoHub : Hub
{
    public string Echo(string value)
    {
        return value;
    }
}