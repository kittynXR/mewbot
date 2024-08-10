import React from 'react';
import SettingsForm from './SettingsForm';

const Settings = () => {
    return (
        <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow-md">
            <h2 className="text-2xl font-bold mb-4 dark:text-white">Settings</h2>
            <SettingsForm />
        </div>
    );
};

export default Settings;