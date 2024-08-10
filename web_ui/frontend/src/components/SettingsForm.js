import React, { useState, useEffect } from 'react';

const SettingsForm = () => {
    const [config, setConfig] = useState({
        twitch_bot_username: '',
        twitch_user_id: '',
        twitch_channel_to_join: '',
        twitch_client_id: '',
        twitch_client_secret: '',
        twitch_irc_oauth_token: '',
        vrchat_auth_cookie: '',
        discord_token: '',
        discord_client_id: '',
        discord_guild_id: '',
        openai_secret: '',
        anthropic_secret: '',
        log_level: 'INFO',
        web_ui_host: '',
        web_ui_port: 3333,
        additional_streams: ['', '', '', ''],
    });
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);
    const [success, setSuccess] = useState(false);

    useEffect(() => {
        fetchConfig();
    }, []);

    const fetchConfig = async () => {
        try {
            const response = await fetch('/api/config');
            if (!response.ok) {
                throw new Error('Failed to fetch configuration');
            }
            const data = await response.json();
            setConfig(prevConfig => ({
                ...prevConfig,
                ...data,
                additional_streams: data.additional_streams && data.additional_streams.length === 4
                    ? data.additional_streams
                    : [...(data.additional_streams || []), ...Array(4 - (data.additional_streams || []).length).fill('')],
            }));
            setLoading(false);
        } catch (err) {
            setError(err.message);
            setLoading(false);
        }
    };

    const handleInputChange = (e) => {
        const { name, value } = e.target;
        setConfig(prevConfig => ({
            ...prevConfig,
            [name]: value
        }));
    };

    const handleStreamChange = (index, value) => {
        setConfig(prevConfig => {
            const newStreams = [...prevConfig.additional_streams];
            newStreams[index] = value;
            return { ...prevConfig, additional_streams: newStreams };
        });
    };

    const handleSubmit = async (e) => {
        e.preventDefault();
        setLoading(true);
        setError(null);
        setSuccess(false);

        try {
            const response = await fetch('/api/config', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(config),
            });

            if (!response.ok) {
                throw new Error('Failed to update configuration');
            }

            setSuccess(true);
            setLoading(false);
        } catch (err) {
            setError(err.message);
            setLoading(false);
        }
    };

    if (loading) {
        return <div className="text-white">Loading...</div>;
    }

    return (
        <form onSubmit={handleSubmit} className="space-y-6 text-gray-200">
            <div className="space-y-4">
                <h2 className="text-xl font-bold">Twitch Settings</h2>
                <div className="grid grid-cols-2 gap-4">
                    <div>
                        <label htmlFor="twitch_bot_username">Bot Username</label>
                        <input
                            type="text"
                            id="twitch_bot_username"
                            name="twitch_bot_username"
                            value={config.twitch_bot_username}
                            onChange={handleInputChange}
                            className="w-full p-2 border rounded bg-gray-700 text-white"
                        />
                    </div>
                    <div>
                        <label htmlFor="twitch_channel_to_join">Channel to Join</label>
                        <input
                            type="text"
                            id="twitch_channel_to_join"
                            name="twitch_channel_to_join"
                            value={config.twitch_channel_to_join}
                            onChange={handleInputChange}
                            className="w-full p-2 border rounded bg-gray-700 text-white"
                        />
                    </div>
                    <div>
                        <label htmlFor="twitch_client_id">Client ID</label>
                        <input
                            type="text"
                            id="twitch_client_id"
                            name="twitch_client_id"
                            value={config.twitch_client_id}
                            onChange={handleInputChange}
                            className="w-full p-2 border rounded bg-gray-700 text-white"
                        />
                    </div>
                    <div>
                        <label htmlFor="twitch_client_secret">Client Secret</label>
                        <input
                            type="password"
                            id="twitch_client_secret"
                            name="twitch_client_secret"
                            value={config.twitch_client_secret}
                            onChange={handleInputChange}
                            className="w-full p-2 border rounded bg-gray-700 text-white"
                        />
                    </div>
                    <div>
                        <label htmlFor="twitch_irc_oauth_token">IRC OAuth Token</label>
                        <input
                            type="password"
                            id="twitch_irc_oauth_token"
                            name="twitch_irc_oauth_token"
                            value={config.twitch_irc_oauth_token}
                            onChange={handleInputChange}
                            className="w-full p-2 border rounded bg-gray-700 text-white"
                        />
                    </div>
                </div>
            </div>

            <div className="space-y-4">
                <h2 className="text-xl font-bold">Discord Settings</h2>
                <div className="grid grid-cols-2 gap-4">
                    <div>
                        <label htmlFor="discord_token">Bot Token</label>
                        <input
                            type="password"
                            id="discord_token"
                            name="discord_token"
                            value={config.discord_token}
                            onChange={handleInputChange}
                            className="w-full p-2 border rounded bg-gray-700 text-white"
                        />
                    </div>
                    <div>
                        <label htmlFor="discord_client_id">Client ID</label>
                        <input
                            type="text"
                            id="discord_client_id"
                            name="discord_client_id"
                            value={config.discord_client_id}
                            onChange={handleInputChange}
                            className="w-full p-2 border rounded bg-gray-700 text-white"
                        />
                    </div>
                    <div>
                        <label htmlFor="discord_guild_id">Guild ID</label>
                        <input
                            type="text"
                            id="discord_guild_id"
                            name="discord_guild_id"
                            value={config.discord_guild_id}
                            onChange={handleInputChange}
                            className="w-full p-2 border rounded bg-gray-700 text-white"
                        />
                    </div>
                </div>
            </div>

            <div className="space-y-4">
                <h2 className="text-xl font-bold">VRChat Settings</h2>
                <div>
                    <label htmlFor="vrchat_auth_cookie">Auth Cookie</label>
                    <input
                        type="password"
                        id="vrchat_auth_cookie"
                        name="vrchat_auth_cookie"
                        value={config.vrchat_auth_cookie}
                        onChange={handleInputChange}
                        className="w-full p-2 border rounded bg-gray-700 text-white"
                    />
                </div>
            </div>

            <div className="space-y-4">
                <h2 className="text-xl font-bold">AI Settings</h2>
                <div className="grid grid-cols-2 gap-4">
                    <div>
                        <label htmlFor="openai_secret">OpenAI Secret</label>
                        <input
                            type="password"
                            id="openai_secret"
                            name="openai_secret"
                            value={config.openai_secret}
                            onChange={handleInputChange}
                            className="w-full p-2 border rounded bg-gray-700 text-white"
                        />
                    </div>
                    <div>
                        <label htmlFor="anthropic_secret">Anthropic Secret</label>
                        <input
                            type="password"
                            id="anthropic_secret"
                            name="anthropic_secret"
                            value={config.anthropic_secret}
                            onChange={handleInputChange}
                            className="w-full p-2 border rounded bg-gray-700 text-white"
                        />
                    </div>
                </div>
            </div>

            <div className="space-y-4">
                <h2 className="text-xl font-bold">General Settings</h2>
                <div className="grid grid-cols-2 gap-4">
                    <div>
                        <label htmlFor="log_level">Log Level</label>
                        <select
                            id="log_level"
                            name="log_level"
                            value={config.log_level}
                            onChange={handleInputChange}
                            className="w-full p-2 border rounded bg-gray-700 text-white"
                        >
                            <option value="ERROR">ERROR</option>
                            <option value="WARN">WARN</option>
                            <option value="INFO">INFO</option>
                            <option value="DEBUG">DEBUG</option>
                            <option value="VERBOSE">VERBOSE</option>
                        </select>
                    </div>
                    <div>
                        <label htmlFor="web_ui_host">Web UI Host</label>
                        <input
                            type="text"
                            id="web_ui_host"
                            name="web_ui_host"
                            value={config.web_ui_host}
                            onChange={handleInputChange}
                            className="w-full p-2 border rounded bg-gray-700 text-white"
                        />
                    </div>
                    <div>
                        <label htmlFor="web_ui_port">Web UI Port</label>
                        <input
                            type="number"
                            id="web_ui_port"
                            name="web_ui_port"
                            value={config.web_ui_port}
                            onChange={handleInputChange}
                            className="w-full p-2 border rounded bg-gray-700 text-white"
                        />
                    </div>
                </div>
            </div>

            <div className="space-y-4">
                <h2 className="text-xl font-bold">Additional Streams</h2>
                <div className="grid grid-cols-2 gap-4">
                    {config.additional_streams.map((stream, index) => (
                        <div key={index}>
                            <label htmlFor={`additional_stream_${index}`}>Stream {index + 1}</label>
                            <input
                                type="text"
                                id={`additional_stream_${index}`}
                                value={stream}
                                onChange={(e) => handleStreamChange(index, e.target.value)}
                                className="w-full p-2 border rounded bg-gray-700 text-white"
                            />
                        </div>
                    ))}
                </div>
            </div>

            {error && (
                <div className="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded relative" role="alert">
                    <strong className="font-bold">Error!</strong>
                    <span className="block sm:inline"> {error}</span>
                </div>
            )}

            {success && (
                <div className="bg-green-100 border border-green-400 text-green-700 px-4 py-3 rounded relative" role="alert">
                    <strong className="font-bold">Success!</strong>
                    <span className="block sm:inline"> Configuration updated successfully.</span>
                </div>
            )}

            <button
                type="submit"
                disabled={loading}
                className="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
            >
                {loading ? 'Saving...' : 'Save Configuration'}
            </button>
        </form>
    );
};

export default SettingsForm;