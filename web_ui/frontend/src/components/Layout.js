import React, { useState } from 'react';
import { Home, Settings, Layout as LayoutIcon, ChevronLeft, ChevronRight } from 'lucide-react';
import Chatbox from './Chatbox';

const Sidebar = ({ activeItem, setActiveItem, isCollapsed, toggleSidebar }) => {
    const menuItems = [
        { name: 'Dashboard', icon: Home },
        { name: 'OBS Overlay', icon: LayoutIcon },
        { name: 'Settings', icon: Settings },
    ];

    return (
        <div className={`bg-gray-800 text-white ${isCollapsed ? 'w-16' : 'w-64'} space-y-6 py-7 px-2 absolute inset-y-0 left-0 transform ${isCollapsed ? '-translate-x-0' : '-translate-x-full'} md:relative md:translate-x-0 transition duration-200 ease-in-out`}>
            <nav>
                {menuItems.map((item) => (
                    <button
                        key={item.name}
                        className={`block w-full text-left py-2.5 px-4 rounded transition duration-200 ${
                            activeItem === item.name ? 'bg-gray-700' : 'hover:bg-gray-700'
                        }`}
                        onClick={() => setActiveItem(item.name)}
                    >
                        <item.icon className="inline-block mr-2 h-5 w-5" />
                        {!isCollapsed && item.name}
                    </button>
                ))}
            </nav>
            <button
                onClick={toggleSidebar}
                className="absolute top-1/2 -right-3 bg-gray-700 text-white p-1 rounded-full"
            >
                {isCollapsed ? <ChevronRight size={20} /> : <ChevronLeft size={20} />}
            </button>
        </div>
    );
};

const Layout = ({ children, activeView, setActiveView }) => {
    const [isCollapsed, setIsCollapsed] = useState(false);
    const [messages, setMessages] = useState([]); // This should be populated with actual Twitch chat messages

    const toggleSidebar = () => setIsCollapsed(!isCollapsed);

    return (
        <div className="flex h-screen bg-gray-900 text-white">
            <Sidebar
                activeItem={activeView}
                setActiveItem={setActiveView}
                isCollapsed={isCollapsed}
                toggleSidebar={toggleSidebar}
            />
            <div className="flex-1 flex flex-col overflow-hidden">
                <header className="bg-gray-800 shadow-md p-4">
                    <h1 className="text-2xl font-bold text-white">MewBot Web UI</h1>
                </header>
                <div className="flex-1 flex overflow-hidden">
                    <div className="w-1/4 p-4 border-r border-gray-700">
                        <Chatbox messages={messages} />
                    </div>
                    <main className="flex-1 p-4 overflow-y-auto">
                        {children}
                    </main>
                </div>
            </div>
        </div>
    );
};

export default Layout;