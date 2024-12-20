import React, { useReducer, useEffect, useCallback, useState } from 'react';
import TwitchPlayer from './TwitchPlayer';
import VRChatWorldStatus from './VRChatWorldStatus';
import useWebSocket from './useWebSocket';
import Chat from './Chat';
import BotStatus from './BotStatus';

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
    obsStatus: false,
    obsInstances: [],
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
        case 'SET_OBS_STATUS':
            return { ...state, obsStatus: action.payload };
        case 'SET_OBS_INSTANCES':
            return { ...state, obsInstances: action.payload };
        case 'SET_INSTANCE_NAME':
            return { ...state, instanceName: action.payload };
        case 'SET_SCENE_NAME':
            return { ...state, sceneName: action.payload };
        case 'SET_SOURCE_NAME':
            return { ...state, sourceName: action.payload };
        case 'SET_ENABLED':
            return { ...state, enabled: action.payload };
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
                if (data.message) {
                    dispatch({ type: 'SET_BOT_STATUS', payload: data.message });
                }
                if (data.update_data) {
                    const {
                        uptime,
                        vrchat_world,
                        recent_messages,
                        twitch_status,
                        discord_status,
                        vrchat_status,
                        obs_status,
                        obs_instances,
                        instance_name,
                        scene_name,
                        source_name,
                        enabled
                    } = data.update_data;

                    dispatch({ type: 'SET_UPTIME', payload: uptime || '-' });
                    dispatch({ type: 'SET_VRC_WORLD', payload: vrchat_world });
                    dispatch({ type: 'SET_TWITCH_STATUS', payload: twitch_status });
                    dispatch({ type: 'SET_DISCORD_STATUS', payload: discord_status });
                    dispatch({ type: 'SET_VRCHAT_STATUS', payload: vrchat_status });
                    dispatch({ type: 'SET_OBS_STATUS', payload: obs_status });

                    if (recent_messages) {
                        dispatch({ type: 'SET_RECENT_MESSAGES', payload: recent_messages });
                    }
                    if (obs_instances) {
                        dispatch({ type: 'SET_OBS_INSTANCES', payload: obs_instances });
                    }

                    // Handle new fields from the message structure
                    if (instance_name) {
                        dispatch({ type: 'SET_INSTANCE_NAME', payload: instance_name });
                    }
                    if (scene_name) {
                        dispatch({ type: 'SET_SCENE_NAME', payload: scene_name });
                    }
                    if (source_name) {
                        dispatch({ type: 'SET_SOURCE_NAME', payload: source_name });
                    }
                    if (enabled !== undefined) {
                        dispatch({ type: 'SET_ENABLED', payload: enabled });
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
            case 'obs_update':
                if (data.update_data && data.update_data.obs_instances) {
                    dispatch({ type: 'SET_OBS_INSTANCES', payload: data.update_data.obs_instances });
                }
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

    const handleShareWorld = useCallback(() => {
        if (state.currentVRCWorld && isConnected) {
            sendMessage({ type: 'shareWorld', world: state.currentVRCWorld });
        } else if (!isConnected) {
            dispatch({ type: 'WS_ERROR', payload: 'WebSocket is not connected' });
        } else {
            dispatch({ type: 'WS_ERROR', payload: 'No VRChat world to share' });
        }
    }, [state.currentVRCWorld, sendMessage, isConnected]);

    /// JSX Starts here
    return (
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            {wsConnectionError && (
                <div className="col-span-full bg-red-500 text-white p-4 mb-4">
                    WebSocket Error: {wsConnectionError}
                </div>
            )}
            <div className="bg-gray-800 p-6 rounded-lg shadow-md md:col-span-3">
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
            <div className="md:col-span-2">
                <VRChatWorldStatus
                    currentVRCWorld={state.currentVRCWorld}
                    vrchatStatus={state.vrchatStatus}
                    handleShareWorld={handleShareWorld}
                />
            </div>
            <div className="md:col-span-1">
                <BotStatus
                    botStatus={state.botStatus}
                    uptime={state.uptime}
                    twitchStatus={state.twitchStatus}
                    discordStatus={state.discordStatus}
                    vrchatStatus={state.vrchatStatus}
                />
            </div>
            <div className="md:col-span-3">
                <Chat
                    state={state}
                    dispatch={dispatch}
                    sendMessage={sendMessage}
                    isConnected={isConnected}
                />
            </div>
            <div className="bg-gray-800 p-6 rounded-lg shadow-md md:col-span-3">
                <h2 className="text-2xl font-bold mb-4 text-white">Recent Messages</h2>
                <ul className="list-disc list-inside text-gray-300">
                    {state.recentMessages.map((message, index) => (
                        <li key={index}>{message}</li>
                    ))}
                </ul>
            </div>
            {state.additionalStreams.length > 0 && (
                <div className="bg-gray-800 p-6 rounded-lg shadow-md md:col-span-3">
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