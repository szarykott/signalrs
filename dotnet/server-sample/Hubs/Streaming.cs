// Licensed to the .NET Foundation under one or more agreements.
// The .NET Foundation licenses this file to you under the MIT license.

using System;
using System.Collections.Generic;
using System.Reactive.Linq;
using System.Threading.Channels;
using System.Threading.Tasks;
using Microsoft.AspNetCore.SignalR;

namespace server_sample.Hubs;

public class Streaming : Hub
{
    public async IAsyncEnumerable<int> AsyncEnumerableCounter(int count, double delay)
    {
        for (var i = 0; i < count; i++)
        {
            yield return i;
            await Task.Delay(TimeSpan.FromMilliseconds(delay));
        }
    }

    public ChannelReader<int> ChannelCounter(int count, double delay)
    {
        var channel = Channel.CreateUnbounded<int>();

        Task.Run(async () =>
        {
            for (var i = 0; i < count; i++)
            {
                await channel.Writer.WriteAsync(i);
                await Task.Delay(TimeSpan.FromMilliseconds(delay));
            }

            channel.Writer.TryComplete();
        });

        return channel.Reader;
    }
}
