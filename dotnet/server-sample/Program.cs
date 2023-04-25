using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using server_sample.Hubs;

internal class Program
{
    private static void Main(string[] args)
    {
        var builder = WebApplication.CreateBuilder(args);
        builder.WebHost.UseUrls("http://localhost:5261");

        // Add services to the container.

        builder.Services.AddControllers();
        // Learn more about configuring Swagger/OpenAPI at https://aka.ms/aspnetcore/swashbuckle
        builder.Services.AddEndpointsApiExplorer();
        builder.Services.AddSwaggerGen();
        builder.Services.AddSignalR();
        builder.Services.AddLogging(builder =>
        {
            builder.SetMinimumLevel(LogLevel.Trace);
            builder.AddConsole();
        });

        var app = builder.Build();

        app.UseHttpLogging();

        app.MapControllers();
        app.MapHub<Chat>("/chat");
        app.MapHub<DynamicChat>("/dynamic-chat");
        app.MapHub<EchoHub>("/echo");
        app.MapHub<HubTChat>("/hub-t-chat");
        app.MapHub<Streaming>("/streaming");
        app.MapHub<UploadHub>("/upload");

        app.Run();
    }
}