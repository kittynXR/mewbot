import React from 'react';

const BotStatus = ({ botStatus, uptime, twitchStatus, discordStatus, vrchatStatus }) => {
    return (
        <div className="bg-gray-800 p-6 rounded-lg shadow-md">
            <h2 className="text-2xl font-bold mb-4 text-white">Bot Status</h2>
            <p className="text-gray-300">Status: <span className="font-bold text-white">{botStatus}</span></p>
            <p className="text-gray-300">Uptime: <span className="font-bold text-white">{uptime}</span></p>

            <h2 className="text-2xl font-bold mt-6 mb-4 text-white">Connection Status</h2>
            <p className="text-gray-300">Twitch: <span
                className={`font-bold ${twitchStatus ? 'text-green-500' : 'text-red-500'}`}>
        {twitchStatus ? 'Connected' : 'Disconnected'}
      </span></p>
            <p className="text-gray-300">Discord: <span
                className={`font-bold ${discordStatus ? 'text-green-500' : 'text-red-500'}`}>
        {discordStatus ? 'Connected' : 'Disconnected'}
      </span></p>
            <p className="text-gray-300">VRChat: <span
                className={`font-bold ${vrchatStatus ? 'text-green-500' : 'text-red-500'}`}>
        {vrchatStatus ? 'Connected' : 'Disconnected'}
      </span></p>
        </div>
    );
};

export default BotStatus;