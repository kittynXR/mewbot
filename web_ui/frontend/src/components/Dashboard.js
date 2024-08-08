import React, { useState, useEffect, useCallback } from 'react';
import { Share } from 'lucide-react';

const Dashboard = () => {
    const [botStatus, setBotStatus] = useState('Unknown');
    const [uptime, setUptime] = useState('-');
    const [currentVRCWorld, setCurrentVRCWorld] = useState(null);
    const [recentMessages, setRecentMessages] = useState([]);
    const [chatMessage, setChatMessage] = useState('');
    const [chatDestination, setChatDestination] = useState({
        oscTextbox: false,
        twitchChat: false
    });

    const [ws, setWs] = useState(null);

    useEffect(() => {
        const socket = new WebSocket('ws://localhost:3000/ws');

        socket.onopen = () => {
            console.log('WebSocket connection established');
        };

        socket.onmessage = (event) => {
            const data = JSON.parse(event.data);
            if (data.type === 'update') {
                setBotStatus(data.data.bot_status);
                setUptime(data.data.uptime);
                setCurrentVRCWorld(data.data.vrchat_world);
                setRecentMessages(data.data.recent_messages);
            }
        };

        socket.onclose = () => {
            console.log('WebSocket connection closed');
        };

        setWs(socket);

        return () => {
            socket.close();
        };
    }, []);

    const handleShareWorld = useCallback(() => {
        if (ws && currentVRCWorld) {
            ws.send(JSON.stringify({ type: 'shareWorld', world: currentVRCWorld }));
        }
    }, [ws, currentVRCWorld]);

    const handleSendChat = useCallback((e) => {
        e.preventDefault();
        if (chatMessage.trim() === '') return;

        if (ws) {
            ws.send(JSON.stringify({
                type: 'sendChat',
                message: chatMessage,
                destination: chatDestination
            }));
        }

        setChatMessage('');
    }, [ws, chatMessage, chatDestination]);

    return (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <div className="bg-gray-800 p-6 rounded-lg shadow-md">
                <h2 className="text-2xl font-bold mb-4 text-white">Bot Status</h2>
                <p className="text-gray-300">Status: <span className="font-bold text-white">{botStatus}</span></p>
                <p className="text-gray-300">Uptime: <span className="font-bold text-white">{uptime}</span></p>
            </div>
            <div className="bg-gray-800 p-6 rounded-lg shadow-md">
                <h2 className="text-2xl font-bold mb-4 text-white">Current VRChat World</h2>
                {currentVRCWorld ? (
                    <>
                        <p className="text-gray-300 mb-2">Name: {currentVRCWorld.name}</p>
                        <p className="text-gray-300 mb-2">Author: {currentVRCWorld.author_name}</p>
                        <p className="text-gray-300 mb-4">Capacity: {currentVRCWorld.capacity}</p>
                    </>
                ) : (
                    <p className="text-gray-300 mb-4">Not in a world</p>
                )}
                <button
                    onClick={handleShareWorld}
                    className="bg-blue-500 hover:bg-blue-600 text-white font-bold py-2 px-4 rounded flex items-center"
                    disabled={!currentVRCWorld}
                >
                    <Share className="mr-2" size={16} />
                    Share with Chat
                </button>
            </div>
            <div className="bg-gray-800 p-6 rounded-lg shadow-md md:col-span-2">
                <h2 className="text-2xl font-bold mb-4 text-white">Chat</h2>
                <form onSubmit={handleSendChat} className="mb-4">
                    <div className="flex items-center mb-2">
                        <button
                            type="button"
                            onClick={() => setChatDestination(prev => ({ ...prev, oscTextbox: !prev.oscTextbox }))}
                            className={`mr-2 px-4 py-2 rounded ${chatDestination.oscTextbox ? 'bg-blue-500' : 'bg-gray-600'}`}
                        >
                            OSC Textbox
                        </button>
                        <button
                            type="button"
                            onClick={() => setChatDestination(prev => ({ ...prev, twitchChat: !prev.twitchChat }))}
                            className={`mr-2 px-4 py-2 rounded ${chatDestination.twitchChat ? 'bg-blue-500' : 'bg-gray-600'}`}
                        >
                            Twitch Chat
                        </button>
                    </div>
                    <div className="flex">
                        <input
                            type="text"
                            value={chatMessage}
                            onChange={(e) => setChatMessage(e.target.value)}
                            className="flex-grow mr-2 px-4 py-2 bg-gray-700 text-white rounded"
                            placeholder="Type your message..."
                        />
                        <button
                            type="submit"
                            className="bg-green-500 hover:bg-green-600 text-white font-bold py-2 px-4 rounded"
                        >
                            Send
                        </button>
                    </div>
                </form>
            </div>
            <div className="bg-gray-800 p-6 rounded-lg shadow-md md:col-span-2">
                <h2 className="text-2xl font-bold mb-4 text-white">Recent Messages</h2>
                <ul className="list-disc list-inside text-gray-300">
                    {recentMessages.map((message, index) => (
                        <li key={index}>{message}</li>
                    ))}
                </ul>
            </div>
        </div>
    );
};

export default Dashboard;