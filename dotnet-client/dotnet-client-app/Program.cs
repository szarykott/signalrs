using System;
using Microsoft.AspNetCore.SignalR.Client;
using System.Threading.Tasks;
using Microsoft.Extensions.Logging;
using System.Collections.Generic;
using System.Threading;

var connection = new HubConnectionBuilder()
    .WithUrl("http://127.0.0.1:8080/chathub")
    .ConfigureLogging(options =>
    {
        options.AddConsole();
        options.AddFilter("Microsoft.AspNetCore.SignalR", LogLevel.Information);
    })
    .Build();

connection.Reconnected += _ =>
{
    Console.WriteLine("Connection restarted");
    return Task.CompletedTask;
};

connection.Closed += async (error) =>
{
    Console.WriteLine("Connection closed");
    await Task.Delay(TimeSpan.FromSeconds(10));
    await connection.StartAsync();
};

Connect:
try
{
    if (connection.State == HubConnectionState.Disconnected)
        await connection.StartAsync();
}
catch (Exception e)
{
    System.Console.WriteLine(e);
    System.Console.WriteLine("Error while trying to connect. Click any key to retry ...");
    Console.ReadKey();
    goto Connect;
}

while (true)
{
    Console.WriteLine("Choose action:");

    Console.WriteLine("0 - close");
    Console.WriteLine("1 - invoke 'add'");
    Console.WriteLine("2 - invoke 'single_result_failure'");
    Console.WriteLine("3 - invoke 'batched'");
    Console.WriteLine("4 - invoke 'stream'");
    Console.WriteLine("5 - invoke 'non_blocking'");
    Console.WriteLine("6 - invoke 'stream_failure'");
    Console.WriteLine("7 - invoke 'stream' cancel");
    Console.WriteLine("8 - invoke 'add_stream'");

    var selection = Console.ReadLine();

    try
    {
        switch (selection)
        {
            case "0":
                goto End;
            case "1":
                await InvokeAdd(connection);
                break;
            case "2":
                await InvokeSingleResultFailure(connection);
                break;
            case "3":
                await InvokeBatched(connection);
                break;
            case "4":
                await InvokeStream(connection);
                break;
            case "5":
                await InvokeNonBlocking(connection);
                break;
            case "6":
                await InvokeStreamFailure(connection);
                break;
            case "7":
                await InvokeStreamCancel(connection);
                break;
            case "8":
                await InvokeClientSideStreaming(connection);
                break;
            default:
                break;
        }
    }
    catch (Exception e)
    {
        Console.WriteLine(e);
        goto Connect;
    }
}

End:;

async Task InvokeAdd(HubConnection connection)
{
    var response = await connection.InvokeAsync<int>("add", 1, 2);
    Console.WriteLine($"'add' returned {response}");
}

async Task InvokeSingleResultFailure(HubConnection connection)
{
    var response = await connection.InvokeAsync<int>("single_result_failure", 1, 2);
    Console.WriteLine($"'single_result_failure' returned {response}");
}

async Task InvokeBatched(HubConnection connection)
{
    var response = await connection.InvokeAsync<List<int>>("batched", 3);
    Console.WriteLine($"'single_result_failure' returned {string.Join(",", response)}");
}

async Task InvokeStream(HubConnection connection)
{
    var response = connection.StreamAsync<int>("stream", 5);

    await foreach (var i in response)
    {
        System.Console.WriteLine($"'stream' next item : {i}");
    }

    Console.WriteLine($"'stream' finished");
}

async Task InvokeNonBlocking(HubConnection connection)
{
    await connection.SendAsync("non_blocking");
    Console.WriteLine($"'non_blocking' returned");
}

async Task InvokeStreamFailure(HubConnection connection)
{
    var response = connection.StreamAsync<int>("stream_failure", 5);

    await foreach (var i in response)
    {
        System.Console.WriteLine($"'stream' next item : {i}");
    }

    Console.WriteLine($"'stream' finished");
}

async Task InvokeStreamCancel(HubConnection connection)
{
    var cts = new CancellationTokenSource(TimeSpan.FromSeconds(2));

    var response = connection.StreamAsync<int>("stream", 5, cts.Token);

    await foreach (var i in response)
    {
        System.Console.WriteLine($"'stream' next item : {i}");
    }

    Console.WriteLine($"'stream' finished");
}

async Task InvokeClientSideStreaming(HubConnection connection)
{
    var response = await connection.InvokeAsync<int>("add_stream", LinesAsync(), LinesAsync());
    Console.WriteLine($"'add_stream' finished with {response}");
}

async IAsyncEnumerable<int> LinesAsync()
{
    for (var i = 0; i < 5; i++)
    {
        await Task.Delay(1000);
        yield return i;
    }
}