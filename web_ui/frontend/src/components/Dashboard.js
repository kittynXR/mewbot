import React, { useState, useEffect, useCallback, useRef } from 'react';
import { Share } from 'lucide-react';
import TwitchPlayer from './TwitchPlayer';

const Dashboard = ({ setTwitchMessages }) => {
    const [botStatus, setBotStatus] = useState('Unknown');
    const [uptime, setUptime] = useState('-');
    const [currentVRCWorld, setCurrentVRCWorld] = useState(null);
    const [recentMessages, setRecentMessages] = useState([]);
    const [chatMessage, setChatMessage] = useState('');
    const [chatDestination, setChatDestination] = useState({
        oscTextbox: false,
        twitchChat: false,
        twitchBot: false,
        twitchBroadcaster: false
    });
    const [twitchStatus, setTwitchStatus] = useState(false);
    const [discordStatus, setDiscordStatus] = useState(false);
    const [vrchatStatus, setVRChatStatus] = useState(false);
    const [twitchChannel, setTwitchChannel] = useState('');
    const [additionalStreams, setAdditionalStreams] = useState([]);
    const [additionalStreamToggles, setAdditionalStreamToggles] = useState([]);
    const [twitchError, setTwitchError] = useState(null);

    const socketRef = useRef(null);

    const connectWebSocket = useCallback(() => {
        if (socketRef.current && socketRef.current.readyState === WebSocket.OPEN) {
            console.log('WebSocket is already connected');
            return;
        }

        console.log('Attempting to connect WebSocket...');
        const socket = new WebSocket(`ws://${window.location.hostname}:3333/ws`);

        socket.onopen = () => {
            console.log('WebSocket connection established');
            socketRef.current = socket;
        };

        socket.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                console.log('Received WebSocket message:', data);

                if (data.type === 'update' || data.type === 'bot_status') {
                    console.log('Received bot status update:', data);
                    setBotStatus(data.message || 'Unknown');
                    if (data.world) {
                        setUptime(data.world.uptime || '-');
                        setCurrentVRCWorld(data.world.vrchat_world || null);
                        setRecentMessages(prevMessages => {
                            const updatedMessages = [...prevMessages, ...(data.world.recent_messages || [])].slice(-10);
                            console.log('Updated recent messages:', updatedMessages);
                            return updatedMessages;
                        });
                        setTwitchStatus(data.world.twitch_status || false);
                        setDiscordStatus(data.world.discord_status || false);
                        setVRChatStatus(data.world.vrchat_status || false);
                    }
                } else if (data.type === 'twitch_message') {
                    console.log('Received Twitch message:', data.message);
                    setTwitchMessages(prevMessages => {
                        const updatedMessages = [...prevMessages, data.message].slice(-500);
                        console.log('Updated Twitch messages:', updatedMessages);
                        return updatedMessages;
                    });
                    setRecentMessages(prevMessages => {
                        const updatedMessages = [...prevMessages, data.message].slice(-10);
                        console.log('Updated recent messages:', updatedMessages);
                        return updatedMessages;
                    });
                } else if (data.type === 'chatSent') {
                    console.log('Chat message sent successfully:', data.message);
                    // You can add any additional handling for sent messages here
                } else {
                    console.log('Received unknown message type:', data.type);
                }
            } catch (error) {
                console.error('Error processing WebSocket message:', error);
            }
        };

        socket.onerror = (error) => {
            console.error('WebSocket error:', error);
        };

        socket.onclose = (event) => {
            console.log('WebSocket connection closed:', event);
            socketRef.current = null;
            setTimeout(() => {
                console.log('Attempting to reconnect WebSocket...');
                connectWebSocket();
            }, 5000); // Attempt to reconnect after 5 seconds
        };
    }, [setTwitchMessages]);

    useEffect(() => {
        console.log('Setting up WebSocket connection...');
        connectWebSocket();
        return () => {
            console.log('Cleaning up WebSocket connection...');
            if (socketRef.current) {
                socketRef.current.close();
            }
        };
    }, [connectWebSocket]);

    useEffect(() => {
        const fetchTwitchInfo = async () => {
            try {
                const [channelResponse, configResponse] = await Promise.all([
                    fetch('/api/twitch-channel'),
                    fetch('/api/config')
                ]);

                if (!channelResponse.ok || !configResponse.ok) {
                    throw new Error('Failed to fetch Twitch information');
                }

                const channelData = await channelResponse.json();
                const configData = await configResponse.json();

                setTwitchChannel(channelData.channel);
                const filteredStreams = configData.additional_streams.filter(stream => stream);
                setAdditionalStreams(filteredStreams);
                setAdditionalStreamToggles(filteredStreams.map(() => false));
                setTwitchError(null);
            } catch (error) {
                console.error('Failed to fetch Twitch info:', error);
                setTwitchError(error.message);
            }
        };

        fetchTwitchInfo();
    }, []);

    const sendWebSocketMessage = useCallback((message) => {
        if (socketRef.current && socketRef.current.readyState === WebSocket.OPEN) {
            socketRef.current.send(JSON.stringify(message));
        } else {
            console.error('WebSocket is not connected');
        }
    }, []);

    const handleSendChat = useCallback((e) => {
        e.preventDefault();
        console.log('Sending chat message:', chatMessage);
        if (chatMessage.trim() === '') return;

        sendWebSocketMessage({
            type: 'sendChat',
            message: chatMessage,
            destination: chatDestination,
            additionalStreams: additionalStreams.filter((_, index) => additionalStreamToggles[index])
        });

        setChatMessage('');
    }, [chatMessage, chatDestination, additionalStreams, additionalStreamToggles, sendWebSocketMessage]);

    const handleShareWorld = useCallback(() => {
        console.log('Sharing world:', currentVRCWorld);
        if (currentVRCWorld) {
            sendWebSocketMessage({ type: 'shareWorld', world: currentVRCWorld });
        }
    }, [currentVRCWorld, sendWebSocketMessage]);

    return (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <div className="bg-gray-800 p-6 rounded-lg shadow-md md:col-span-2">
                <h2 className="text-2xl font-bold mb-4 text-white">Twitch Stream</h2>
                {twitchError ? (
                    <p className="text-red-500">Error loading Twitch stream: {twitchError}</p>
                ) : twitchChannel ? (
                    <>
                        <p className="text-gray-300 mb-2">Channel: {twitchChannel}</p>
                        <div className="w-full h-0 pb-[56.25%] relative">
                            <div className="absolute inset-0">
                                <TwitchPlayer channel={twitchChannel} />
                            </div>
                        </div>
                    </>
                ) : (
                    <p className="text-gray-300">Loading Twitch stream...</p>
                )}
            </div>
            <div className="bg-gray-800 p-6 rounded-lg shadow-md">
                <h2 className="text-2xl font-bold mb-4 text-white">Bot Status</h2>
                <p className="text-gray-300">Status: <span className="font-bold text-white">{botStatus}</span></p>
                <p className="text-gray-300">Uptime: <span className="font-bold text-white">{uptime}</span></p>
            </div>
            <div className="bg-gray-800 p-6 rounded-lg shadow-md">
                <h2 className="text-2xl font-bold mb-4 text-white">Connection Status</h2>
                <p className="text-gray-300">Twitch: <span className={`font-bold ${twitchStatus ? 'text-green-500' : 'text-red-500'}`}>{twitchStatus ? 'Connected' : 'Disconnected'}</span></p>
                <p className="text-gray-300">Discord: <span className={`font-bold ${discordStatus ? 'text-green-500' : 'text-red-500'}`}>{discordStatus ? 'Connected' : 'Disconnected'}</span></p>
                <p className="text-gray-300">VRChat: <span className={`font-bold ${vrchatStatus ? 'text-green-500' : 'text-red-500'}`}>{vrchatStatus ? 'Connected' : 'Disconnected'}</span></p>
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
                    <Share className="mr-2" size={16}/>
                    Share with Chat
                </button>
            </div>
            <div className="bg-gray-800 p-6 rounded-lg shadow-md md:col-span-2">
                <h2 className="text-2xl font-bold mb-4 text-white">Chat</h2>
                <form onSubmit={handleSendChat} className="mb-4">
                    <div className="flex flex-wrap items-center mb-2">
                        <button
                            type="button"
                            onClick={() => setChatDestination(prev => ({...prev, oscTextbox: !prev.oscTextbox}))}
                            className={`mr-2 mb-2 px-4 py-2 rounded ${chatDestination.oscTextbox ? 'bg-blue-500' : 'bg-gray-600'}`}
                        >
                            OSC Textbox
                        </button>
                        <button
                            type="button"
                            onClick={() => setChatDestination(prev => ({...prev, twitchChat: !prev.twitchChat}))}
                            className={`mr-2 mb-2 px-4 py-2 rounded ${chatDestination.twitchChat ? 'bg-blue-500' : 'bg-gray-600'}`}
                        >
                            Twitch Chat
                        </button>
                        <button
                            type="button"
                            onClick={() => setChatDestination(prev => ({...prev, twitchBot: !prev.twitchBot}))}
                            className={`mr-2 mb-2 px-4 py-2 rounded ${chatDestination.twitchBot ? 'bg-blue-500' : 'bg-gray-600'}`}
                        >
                            As Bot
                        </button>
                        <button
                            type="button"
                            onClick={() => setChatDestination(prev => ({...prev, twitchBroadcaster: !prev.twitchBroadcaster}))}
                            className={`mr-2 mb-2 px-4 py-2 rounded ${chatDestination.twitchBroadcaster ? 'bg-blue-500' : 'bg-gray-600'}`}
                        >
                            As Broadcaster
                        </button>
                        {additionalStreams.map((stream, index) => (
                            <button
                                key={index}
                                type="button"
                                onClick={() => setAdditionalStreamToggles(prev => {
                                    const newToggles = [...prev];
                                    newToggles[index] = !newToggles[index];
                                    return newToggles;
                                })}
                                className={`mr-2 mb-2 px-4 py-2 rounded ${additionalStreamToggles[index] ? 'bg-blue-500' : 'bg-gray-600'}`}
                            >
                                {stream}
                            </button>
                        ))}
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
            {additionalStreams.length > 0 && (
                <div className="bg-gray-800 p-6 rounded-lg shadow-md md:col-span-2">
                    <h2 className="text-2xl font-bold mb-4 text-white">Additional Streams</h2>
                    <div className={`grid ${additionalStreams.length === 1 ? 'grid-cols-1' : 'grid-cols-1 md:grid-cols-2'} gap-4`}>
                        {additionalStreams.map((stream, index) => (
                            <div key={index} className="w-full h-0 pb-[56.25%] relative">
                                <div className="absolute inset-0">
                                    <TwitchPlayer channel={stream} />
                                </div>
                            </div>
                        ))}
                    </div>
                </div>
            )}
        </div>
    );
};

export default Dashboard;