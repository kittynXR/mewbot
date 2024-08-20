import React, { useState, useEffect, useCallback } from 'react';
import useWebSocket from './useWebSocket';

const OBSControl = () => {
    const [obsInstances, setOBSInstances] = useState([]);
    const [error, setError] = useState(null);

    const handleWebSocketMessage = useCallback((data) => {
        console.log("Full WebSocket message in OBSControl:", data);
        if (data.type === 'update' && data.update_data && data.update_data.obs_instances) {
            const processedInstances = data.update_data.obs_instances.map(instance => ({
                id: instance.name,
                name: instance.name,
                scenes: instance.scenes,
                currentScene: instance.current_scene,
                selectedScene: instance.current_scene,
                sources: instance.sources
            }));
            console.log("Processed OBS instances:", processedInstances);
            setOBSInstances(processedInstances);
        }
    }, []);

    const handleWebSocketError = useCallback((error) => {
        console.error('WebSocket error:', error);
    }, []);

    const { sendMessage, isConnected } = useWebSocket(
        `ws://${window.location.hostname}:3333/ws`,
        handleWebSocketMessage,
        handleWebSocketError
    );

    useEffect(() => {
        if (isConnected) {
            sendMessage({ type: 'get_obs_info' });
        }
    }, [isConnected, sendMessage]);

    useEffect(() => {
        console.log('OBS Instances updated:', obsInstances);
    }, [obsInstances]);

    const handleSceneSelect = (instanceId, sceneName) => {
        setOBSInstances(prevInstances =>
            prevInstances.map(instance =>
                instance.id === instanceId ? { ...instance, selectedScene: sceneName } : instance
            )
        );
    };

    const handleSceneChange = (instanceId, sceneName) => {
        if (isConnected) {
            sendMessage({ type: 'change_scene', instanceId, sceneName });
        }
    };

    const handleSourceToggle = (instanceId, sceneName, sourceName, isEnabled) => {
        if (isConnected) {
            sendMessage({ type: 'toggle_source', instanceId, sceneName, sourceName, isEnabled });
        }
    };

    const handleSourceRefresh = (instanceId, sceneName, sourceName) => {
        sendMessage({ type: 'refresh_source', instanceId, sceneName, sourceName });
    };

    if (error) {
        return <div>Error loading OBS data: {error}</div>;
    }

    if (obsInstances.length === 0) {
        return <div>No OBS instances available</div>;
    }

    return (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {obsInstances.map(instance => (
                <div key={instance.id} className="bg-gray-800 p-6 rounded-lg shadow-md">
                    <h2 className="text-2xl font-bold mb-4 text-white">{instance.name}</h2>

                    <div className="mb-4">
                        <h3 className="text-xl font-semibold mb-2 text-white">Scenes</h3>
                        <div className="flex items-center">
                            <select
                                className="w-full p-2 bg-gray-700 text-white rounded mr-2"
                                value={instance.selectedScene}
                                onChange={(e) => handleSceneSelect(instance.id, e.target.value)}
                            >
                                {instance.scenes.map(scene => (
                                    <option key={scene} value={scene}>{scene}</option>
                                ))}
                            </select>
                            <button
                                className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600"
                                onClick={() => handleSceneChange(instance.id)}
                            >
                                Set Active
                            </button>
                        </div>
                        <p className="mt-2 text-sm text-gray-400">
                            Current active scene: <span className="font-semibold">{instance.currentScene}</span>
                        </p>
                    </div>

                    <div>
                        <h3 className="text-xl font-semibold mb-2 text-white">Sources for {instance.selectedScene}</h3>
                        {instance.sources[instance.selectedScene]?.map(source => (
                            <div key={source.name} className="flex items-center justify-between mb-2">
                                <span className="text-white">{source.name}</span>
                                <div>
                                    <button
                                        className={`px-2 py-1 rounded mr-2 ${source.isEnabled ? 'bg-green-500' : 'bg-red-500'}`}
                                        onClick={() => handleSourceToggle(instance.id, instance.selectedScene, source.name, !source.isEnabled)}
                                    >
                                        {source.isEnabled ? 'On' : 'Off'}
                                    </button>
                                    {source.type === 'browser_source' && (
                                        <button
                                            className="px-2 py-1 rounded bg-blue-500"
                                            onClick={() => handleSourceRefresh(instance.id, instance.selectedScene, source.name)}
                                        >
                                            Refresh
                                        </button>
                                    )}
                                </div>
                            </div>
                        ))}
                    </div>
                </div>
            ))}
        </div>
    );
};

export default OBSControl;