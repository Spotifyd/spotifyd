#!/bin/sh
# Author: Sven Lechner (SirWindfield)
# License: GPLv3

# Copies all hooks into the .git/hooks directory
cp hooks/* .git/hooks
SPTConfiguration *configuration =
    [[SPTConfiguration alloc] initWithClientID:@"your_client_id" redirectURL:[NSURL urlWithString:@"your_redirect_uri"]];

// Set these urls to your backend which contains the secret to exchange for an access token
configuration.tokenSwapURL = [NSURL URLWithString: @"http://[your_server]/swap"];
configuration.tokenRefreshURL = [NSURL URLWithString: @"http://[your_server]/refresh"]


self.sessionManager = [SPTSessionManager sessionManagerWithConfiguration:configuration delegate:self]



- (BOOL)application:(UIApplication *)application openURL:(NSURL *)URL options:(NSDictionary<UIApplicationOpenURLOptionsKey, id> *)options
{
    [self.sessionManager application:application openURL:URL options:options];
}


- (void)sessionManager:(SPTSessionManager *)manager didInitiateSession:(SPTSession *)session
{
    self.appRemote.connectionParams.accessToken = session.accessToken
}



/*
Scopes let you specify exactly what types of data your application wants to
access, and the set of scopes you pass in your call determines what access
permissions the user is asked to grant.
For more information, see https://developer.spotify.com/web-api/using-scopes/.
*/
SPTScope scope = SPTUserFollowReadScope | SPTAppRemoteControlScope;

if (@available(iOS 11, *)) {
    // Use this on iOS 11 and above to take advantage of SFAuthenticationSession
    [self.sessionManager initiateSessionWithScope:scope options:SPTDefaultAuthorizationOption];
} else {
    // Use this on iOS versions < 11 to use SFSafariViewController
    [self.sessionManager initiateSessionWithScope:scope options:SPTDefaultAuthorizationOption presentingViewController:self];
}








// Optional: If you plan to connect SPTAppRemote you can start playback during authorization by setting playURI to a non-nil string. If Spotify is already playing it will continue playing even though a URI is provided.
configuration.playURI = "";