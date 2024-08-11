import React, { useState } from 'react';
import Layout from './components/Layout';
import Dashboard from './components/Dashboard';
import OBSOverlay from './components/OBSOverlay';
import Settings from './components/Settings';
import ErrorBoundary from './components/ErrorBoundary';

const App = () => {
    const [activeView, setActiveView] = useState('Dashboard');
    const [twitchMessages, setTwitchMessages] = useState([]);

    const renderView = () => {
        switch (activeView) {
            case 'Dashboard':
                return (
                    <ErrorBoundary>
                        <Dashboard setTwitchMessages={setTwitchMessages} twitchMessages={twitchMessages} />
                    </ErrorBoundary>
                );
            case 'OBS Overlay':
                return <ErrorBoundary><OBSOverlay /></ErrorBoundary>;
            case 'Settings':
                return <ErrorBoundary><Settings /></ErrorBoundary>;
            default:
                return (
                    <ErrorBoundary>
                        <Dashboard setTwitchMessages={setTwitchMessages} twitchMessages={twitchMessages} />
                    </ErrorBoundary>
                );
        }
    };

    return (
        <div className="min-h-screen bg-gray-900 text-white">
            <Layout activeView={activeView} setActiveView={setActiveView} twitchMessages={twitchMessages}>
                {renderView()}
            </Layout>
        </div>
    );
};

export default App;