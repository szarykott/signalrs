using System;
using Microsoft.AspNetCore.SignalR.Client;
using System.Threading.Tasks;
using Microsoft.Extensions.Logging;

var connection = new HubConnectionBuilder()
    .WithUrl("http://127.0.0.1:8080/chathub")
    .ConfigureLogging(options => {
        options.AddConsole();
        options.AddFilter("Microsoft.AspNetCore.SignalR", LogLevel.Information);
    })
    .Build();

connection.Reconnected += _ => {
    Console.WriteLine("Connection restarted");
    return Task.CompletedTask;
};

connection.Closed += async (error) => {
    Console.WriteLine("Connection closed");
    await Task.Delay(TimeSpan.FromSeconds(10));
    await connection.StartAsync();
};

await connection.StartAsync();

while (true) {
    Console.WriteLine("Choose action:");

    Console.WriteLine("0 - close");
    Console.WriteLine("1 - invoke 'add'");

    var selection = Console.ReadLine();

    try {
        switch(selection) {
            case "0":
                goto End;
            case "1":
                await InvokeAdd(connection);
                break;
            default:
                break;
        }
    }
    catch (Exception e) {
        Console.WriteLine(e);
    }
}

End:;

async Task InvokeAdd(HubConnection connection) {
    var response = await connection.InvokeAsync<int>("add", 1, 2);
    Console.WriteLine($"'add' returned {response}");
}