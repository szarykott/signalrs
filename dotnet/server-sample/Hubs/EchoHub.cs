using System;
using System.Threading.Tasks;
using Microsoft.AspNetCore.SignalR;
using Microsoft.Extensions.Logging;

namespace server_sample.Hubs;

public class EchoHub : Hub
{
    private readonly ILogger<EchoHub> logger;

    public EchoHub(ILogger<EchoHub> logger)
    {
        this.logger = logger;
    }

    public string Echo(string value)
    {
        return value;
    }

    public void NoEcho(string value)
    {
        logger.LogInformation(value);
    }
}