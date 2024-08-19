import React, { useReducer, useEffect, useCallback, useState } from 'react';
import TwitchPlayer from './TwitchPlayer';
import VRChatWorldStatus from './VRChatWorldStatus';
import useWebSocket from './useWebSocket';

const initialState = {
    botStatus: 'Unknown',
    uptime: '-',
    currentVRCWorld: null,
    recentMessages: [],
    chatMessage: '',
    chatDestination: {
        oscTextbox: false,
        twitchChat: false,
        twitchBot: false,
        twitchBroadcaster: false
    },
    twitchStatus: false,
    discordStatus: false,
    vrchatStatus: false,
    twitchChannel: '',
    additionalStreams: [],
    additionalStreamToggles: [],
    twitchError: null,
    wsError: null
};

function reducer(state, action) {
    switch (action.type) {
        case 'SET_BOT_STATUS':
            return { ...state, botStatus: action.payload };
        case 'SET_UPTIME':
            return { ...state, uptime: action.payload };
        case 'SET_VRC_WORLD':
            return { ...state, currentVRCWorld: action.payload };
        case 'SET_RECENT_MESSAGES':
            return {
                ...state,
                recentMessages: Array.isArray(action.payload)
                    ? action.payload
                    : (Array.isArray(state.recentMessages) ? state.recentMessages : [])
            };
        case 'SET_CHAT_MESSAGE':
            return { ...state, chatMessage: action.payload };
        case 'SET_CHAT_DESTINATION':
            return { ...state, chatDestination: { ...state.chatDestination, ...action.payload } };
        case 'SET_TWITCH_STATUS':
            return { ...state, twitchStatus: action.payload };
        case 'SET_DISCORD_STATUS':
            return { ...state, discordStatus: action.payload };
        case 'SET_VRCHAT_STATUS':
            return { ...state, vrchatStatus: action.payload };
        case 'SET_TWITCH_CHANNEL':
            return { ...state, twitchChannel: action.payload };
        case 'SET_ADDITIONAL_STREAMS':
            return { ...state, additionalStreams: action.payload, additionalStreamToggles: action.payload.map(() => false) };
        case 'TOGGLE_ADDITIONAL_STREAM':
            return {
                ...state,
                additionalStreamToggles: state.additionalStreamToggles.map((toggle, index) =>
                    index === action.payload ? !toggle : toggle
                )
            };
        case 'SET_TWITCH_ERROR':
            return { ...state, twitchError: action.payload };
        case 'WS_ERROR':
            return { ...state, wsError: action.payload };
        default:
            return state;
    }
}

const Dashboard = ({ setTwitchMessages }) => {
    const [state, dispatch] = useReducer(reducer, initialState);
    const [wsConnectionError, setWsConnectionError] = useState(null);

    const handleWebSocketMessage = useCallback((data) => {
        console.log('Received WebSocket message:', data);

        switch (data.type) {
            case 'update':
            case 'bot_status':
                dispatch({ type: 'SET_BOT_STATUS', payload: data.message || 'Unknown' });
                if (data.world) {
                    dispatch({ type: 'SET_UPTIME', payload: data.world.uptime || '-' });
                    dispatch({ type: 'SET_VRC_WORLD', payload: data.world.vrchat_world });
                    dispatch({ type: 'SET_TWITCH_STATUS', payload: data.world.twitch_status });
                    dispatch({ type: 'SET_DISCORD_STATUS', payload: data.world.discord_status });
                    dispatch({ type: 'SET_VRCHAT_STATUS', payload: data.world.vrchat_status });
                    dispatch({ type: 'SET_OBS_STATUS', payload: data.world.obs_status });
                    if (data.world.recent_messages) {
                        dispatch({ type: 'SET_RECENT_MESSAGES', payload: prevMessages =>
                                [...prevMessages, ...data.world.recent_messages].slice(-10)
                        });
                    }
                }
                break;
            case 'twitch_message':
                setTwitchMessages(prevMessages => [...prevMessages, data.message].slice(-500));
                dispatch({ type: 'SET_RECENT_MESSAGES', payload: prevMessages =>
                        [...prevMessages, data.message].slice(-10)
                });
                break;
            case 'vrchat_world_update':
                dispatch({ type: 'SET_VRC_WORLD', payload: data.world });
                break;
            default:
                console.log('Unhandled message type:', data.type);
        }
    }, [setTwitchMessages]);

    const handleWebSocketError = useCallback((error) => {
        console.error('WebSocket error:', error);
        setWsConnectionError(error);
        dispatch({ type: 'WS_ERROR', payload: error });
    }, []);

    const { sendMessage, isConnected } = useWebSocket(
        `ws://${window.location.hostname}:3333/ws`,
        handleWebSocketMessage,
        handleWebSocketError
    );

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

                dispatch({ type: 'SET_TWITCH_CHANNEL', payload: channelData.channel });
                const filteredStreams = configData.additional_streams.filter(stream => stream);
                dispatch({ type: 'SET_ADDITIONAL_STREAMS', payload: filteredStreams });
                dispatch({ type: 'SET_TWITCH_ERROR', payload: null });
            } catch (error) {
                console.error('Failed to fetch Twitch info:', error);
                dispatch({ type: 'SET_TWITCH_ERROR', payload: error.message });
            }
        };

        fetchTwitchInfo();
    }, []);

    const handleSendChat = useCallback((e) => {
        e.preventDefault();
        if (state.chatMessage.trim() === '' || !isConnected) return;

        sendMessage({
            type: 'sendChat',
            message: state.chatMessage,
            destination: state.chatDestination,
            additionalStreams: state.additionalStreams.filter((_, index) => state.additionalStreamToggles[index])
        });

        dispatch({ type: 'SET_CHAT_MESSAGE', payload: '' });
    }, [state.chatMessage, state.chatDestination, state.additionalStreams, state.additionalStreamToggles, sendMessage, isConnected]);

    const handleShareWorld = useCallback(() => {
        if (state.currentVRCWorld && isConnected) {
            sendMessage({ type: 'shareWorld', world: state.currentVRCWorld });
        } else if (!isConnected) {
            dispatch({ type: 'WS_ERROR', payload: 'WebSocket is not connected' });
        } else {
            dispatch({ type: 'WS_ERROR', payload: 'No VRChat world to share' });
        }
    }, [state.currentVRCWorld, sendMessage, isConnected]);

    return (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {wsConnectionError && (
                <div className="col-span-full bg-red-500 text-white p-4 mb-4">
                    WebSocket Error: {wsConnectionError}
                </div>
            )}
            <div className="bg-gray-800 p-6 rounded-lg shadow-md md:col-span-2">
                <h2 className="text-2xl font-bold mb-4 text-white">Twitch Stream</h2>
                {state.twitchError ? (
                    <p className="text-red-500">Error loading Twitch stream: {state.twitchError}</p>
                ) : state.twitchChannel ? (
                    <>
                        <p className="text-gray-300 mb-2">Channel: {state.twitchChannel}</p>
                        <div className="w-full h-0 pb-[56.25%] relative">
                            <div className="absolute inset-0">
                                <TwitchPlayer channel={state.twitchChannel}/>
                            </div>
                        </div>
                    </>
                ) : (
                    <p className="text-gray-300">Loading Twitch stream...</p>
                )}
            </div>
            <div className="bg-gray-800 p-6 rounded-lg shadow-md">
                <h2 className="text-2xl font-bold mb-4 text-white">Bot Status</h2>
                <p className="text-gray-300">Status: <span className="font-bold text-white">{state.botStatus}</span></p>
                <p className="text-gray-300">Uptime: <span className="font-bold text-white">{state.uptime}</span></p>
            </div>
            <div className="bg-gray-800 p-6 rounded-lg shadow-md">
                <h2 className="text-2xl font-bold mb-4 text-white">Connection Status</h2>
                <p className="text-gray-300">Twitch: <span
                    className={`font-bold ${state.twitchStatus ? 'text-green-500' : 'text-red-500'}`}>
                {state.twitchStatus ? 'Connected' : 'Disconnected'}
            </span></p>
                <p className="text-gray-300">Discord: <span
                    className={`font-bold ${state.discordStatus ? 'text-green-500' : 'text-red-500'}`}>
                {state.discordStatus ? 'Connected' : 'Disconnected'}
            </span></p>
                <p className="text-gray-300">VRChat: <span
                    className={`font-bold ${state.vrchatStatus ? 'text-green-500' : 'text-red-500'}`}>
                {state.vrchatStatus ? 'Connected' : 'Disconnected'}
            </span></p>
            </div>
            <div className="bg-gray-800 p-6 rounded-lg shadow-md">
                <VRChatWorldStatus
                    currentVRCWorld={state.currentVRCWorld}
                    vrchatStatus={state.vrchatStatus}
                    handleShareWorld={handleShareWorld}
                />
            </div>
            <div className="bg-gray-800 p-6 rounded-lg shadow-md md:col-span-2">
                <h2 className="text-2xl font-bold mb-4 text-white">Chat</h2>
                <form onSubmit={handleSendChat} className="mb-4">
                    <div className="flex flex-wrap items-center mb-2">
                        {Object.entries(state.chatDestination).map(([key, value]) => (
                            <button
                                key={key}
                                type="button"
                                onClick={() => dispatch({type: 'SET_CHAT_DESTINATION', payload: {[key]: !value}})}
                                className={`mr-2 mb-2 px-4 py-2 rounded ${value ? 'bg-blue-500' : 'bg-gray-600'}`}
                            >
                                {key.charAt(0).toUpperCase() + key.slice(1)}
                            </button>
                        ))}
                        {state.additionalStreams.map((stream, index) => (
                            <button
                                key={index}
                                type="button"
                                onClick={() => dispatch({type: 'TOGGLE_ADDITIONAL_STREAM', payload: index})}
                                className={`mr-2 mb-2 px-4 py-2 rounded ${state.additionalStreamToggles[index] ? 'bg-blue-500' : 'bg-gray-600'}`}
                            >
                                {stream}
                            </button>
                        ))}
                    </div>
                    <div className="flex">
                        <input
                            type="text"
                            value={state.chatMessage}
                            onChange={(e) => dispatch({type: 'SET_CHAT_MESSAGE', payload: e.target.value})}
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
                    {state.recentMessages.map((message, index) => (
                        <li key={index}>{message}</li>
                    ))}
                </ul>
            </div>
            {state.additionalStreams.length > 0 && (
                <div className="bg-gray-800 p-6 rounded-lg shadow-md md:col-span-2">
                    <h2 className="text-2xl font-bold mb-4 text-white">Additional Streams</h2>
                    <div className={`grid ${state.additionalStreams.length === 1 ? 'grid-cols-1' : 'grid-cols-1 md:grid-cols-2'} gap-4`}>
                        {state.additionalStreams.map((stream, index) => (
                            <div key={index} className="w-full h-0 pb-[56.25%] relative">
                                <div className="absolute inset-0">
                                    <TwitchPlayer channel={stream}/>
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