![Gupax logo](assets/images/banner.png)


## Development Status

[![CI](https://github.com/gupax-io/gupax/actions/workflows/rust.yml/badge.svg)](https://github.com/gupax-io/gupax/actions/workflows/rust.yml) [![Audit](https://github.com/gupax-io/gupax/actions/workflows/rust.yml/badge.svg)](https://github.com/gupax-io/gupax/actions/workflows/audit.yml) 

This repository has a stable release. 
It is intended for end users and offers a friendly and easy user experience, while allowing advanced users to use it for more complex setup.

## Gupax

`Gupax` is a GUI for mining [**Monero**](https://github.com/monero-project/monero) on a decentralized pool using [**P2Pool**](https://github.com/SChernykh/p2pool) and [**XMRig**](https://github.com/xmrig/xmrig). It also brings multiple features like running your own monero node, connecting your miners to a proxy  and optionally participating (but you will want to 😉) in the [XMRvsBeast raffle](https://xmrvsbeast.com). 




[![CI](https://github.com/gupax-io/gupax/actions/workflows/rust.yml/badge.svg)](https://github.com/gupax-io/gupax/actions/workflows/rust.yml) [![Audit](https://github.com/gupax-io/gupax/actions/workflows/rust.yml/badge.svg)](https://github.com/gupax-io/gupax/actions/workflows/audit.yml) 
</div>

## Contents
* [Guide](#Guide)
	- [System requirements](#system-requirements)
	- [Basic setup](#Basic-Setup)
	- [Documentation](#documentation)
* [Tabs](#Tabs)
	- [Simple/Advanced](#simple-advanced)
	- [About](#About)
	- [Status](#Status)
		- [Processes](#Processes)
		- [P2pool](#P2Pool-1)
		- [Benchmarks](#Benchmarks)
	- [Gupax](#Gupax)
	- [Node](#Node)
	- [P2Pool](#P2Pool)
		- [Crawler](#Crawler)
	- [Proxy](#Proxy)
	- [XvB](#XvB)
* [Daemon mode](#daemon-mode)
* [Troubleshooting](#troubleshooting)
	- [Windows](#windows)
	- [Mac OSX](#mac-osx)
* [FAQ](#FAQ)
* [License](#license) 
* [Mirror](#mirror) 
* [Donations](#donation) 

## Guide
### System requirements
- OS: Any [LTS version](https://wikipedia.org/wiki/Long-term_support) of: Windows, Linux, Mac OS  
- CPU that support for OpenGL 3.0+/Vulkan/Metal/DX12 (not needed for daemon mode)
- Architecture: x86_64 for all, arm for macos, linux arm from compilation


### Video guide
See a 3-minute video guide on how to set-up Gupax

https://user-images.githubusercontent.com/101352116/207978455-6ffdc0cc-204c-4594-9a2f-e10c505745bc.mp4

More recent video by Anti MoonBoy:
https://www.youtube.com/watch?v=8_MOQHYRE1c


### Basic setup:

1. [Download the bundled version of Gupax](https://github.com/gupax-io/gupax/releases)
2. Extract
3. Add exception from anti-virus (Windows/Max)
3. Launch Gupax
4. Input your Monero address in the `P2Pool` tab
6. Start P2Pool
7. Start XMRig

You are now mining to your own instance of P2Pool, welcome to the world of decentralized peer-to-peer mining!

### Documentation

A more detailed documentation for advanced users and developers

To come !

## Tabs
Each service tab can be hidden in the settings (Gupax tab). Only the `P2Pool` and `XMRig` tabs are enabled by default to make Gupax more simple to new users.
### Simple/Advanced
Each services offers a simple sub-menu (accessible on the bottom bar). The simple mode will start the service with default working out of the box settings. The advanced mode allows powerful users to configure each services to correspond to their needs.
### About
The About tab will show you a brief description of Gupax, along with the available shortcuts.
![About Tab](assets/images/tabs/about.png)
### Status
This tab has three sub-menus. By default the `Processes` sub-menu will appear.
#### Processes
Monitoring of every services, as well as displaying resources usage of the system. You can hide the column of a service by checking the Gupax tab.
![Processes Tab](assets/images/tabs/processes.png)
#### Payouts
You can see rewards that you were paid. You also have a tool to calculate rewards based on your hashrate. The calculator needs P2Pool to be synced so it has the needed data up to date.
![Payouts Tab](assets/images/tabs/payouts.png)
#### Benchmarks
You can compare your CPU hashrate to the other CPUs of the same model, or even to other models.
![P2Pool Tab](assets/images/tabs/benchmarks.png)
### Gupax
This tab is the settings tab, where you can update Gupax, set where are the binaries for each services, change startup options, the UI scaling, which tabs are hidden and more.
![Gupax Tab](assets/images/tabs/gupax.png)
### Node
Hidden by default. 
You can start a local Monero Node in Gupax, or let it detect and use (if compatible with P2Pool) an already existing local node.
![Node Tab](assets/images/tabs/node.png)
### P2Pool
This is where you set your XMR addresse on which you will receive your rewards. The simple mode will allow you to choose between using a local node or a remote node.
Remote nodes are found by crawling the network. Gupax will do it automatically on startup, so you don't have to wait to start P2Pool. If the Node service is synced, P2Pool will switch to it by default.
![P2Pool Tab](assets/images/tabs/p2pool.png)
#### Crawler
The crawler tab will allow you to tweak the filter of selected remote nodes to connect with P2Pool. For example, you can try to find nodes with a lower latency at the cost of a longer search.
![Crawler Tab](assets/images/tabs/crawler.png)
### XMRig
The service that will start the mining locally. You can set a number of threads if you don't want all the power of your CPU to be dedicated to mining.
![XMRig Tab](assets/images/tabs/xmrig.png)
### Proxy
Hidden by default. 
Use this tab if you want to connect external miners to this instance of Gupax. In case you are participating in the XvB raffle, it will redirect all the hashrate at the decision of the XvB algorithm.
![Proxy Tab](assets/images/tabs/proxy.png)
### XvB
Hidden by default. 
Use this tab to easily split your hashrate between P2Pool and XMRvsBeast, increasing your chances of winning in the raffle while also supporting the Monero network via decentralizing the mining using using p2pool.
[Register](https://xmrvsbeast.com/cgi-bin/p2pool_bonus_submit.cgi) the same XMR address that you use in the P2Pool tab to participate. 
The simple mode will calculate which round you can access based on your current hashrate. The advanced tab allows you to have more manual controls.
![XvB Tab](assets/images/tabs/xvb.png)

## Daemon mode
Gupax can be started as a daemon, without any GUI (intended for CLI only environment).  
To do so, start the executable with the argument `--daemon`.  
The daemon is configurable by the same configuration file that is used by the normal GUI mode that you can find in the following path depending on your OS  
|            |                                              |                                                 |
| ---------- | -------------------------------------------- | ------------------------------------------------|
| Linux      | $XDG_DATA_HOME or $HOME/.local/share/gupax  | /home/alice/.local/state/gupax                 |  
| macOS      | $HOME/Library/Application Support/Gupax     | /Users/Alice/Library/Application Support/Gupax |
| Windows    | {FOLDERID_RoamingAppData}\Gupax             | C:\Users\Alice\AppData\Roaming\Gupax           |


Once started, you can enter the key 's' to print the status of started processes.


## Troubleshooting
If you have any issue, feel free to ask for support in the [xmrvsbeast matrix room](#xmrvsbeast:monero.social) [![Chat on Matrix](https://matrix.to/img/matrix-badge.svg)](https://matrix.to/#/#xmrvsbeast:monero.social) or you can also just [open an issue](https://github.com/Cyrix126/gupax/issues/new/choose) in this repo. You can also contact me through [email](mailto:gupax@baermail.fr).
### Windows
You must add an exception to your antivirus for the directory where Gupax is executed. Follow the step for Windows only, that starts at 30 seconds in this [video](https://user-images.githubusercontent.com/101352116/207978455-6ffdc0cc-204c-4594-9a2f-e10c505745bc.mp4).
### Mac OSX
You must remove Gupax app from quarantine with following command:  
*If you have put Gupax.app in your Applications*  
`xattr -d com.apple.quarantine /Applications/Gupax.app`
### P2Pool connection errors
**TL;DR: Run & use your own Monero node.**

If you are using the [default P2Pool settings](#P2Pool) then you are using a [Remote Monero node](#remote-monero-nodes) that was found by crawling. Using a remote node is convenient but comes at the cost of privacy and reliability. You may encounter connections issues with theses nodes that look like this:
```
2023-01-05 12:27:37.7962 P2PServer peer 23.233.96.72:37888 is ahead on mainchain (height 2792939, your height 2792936). Is your monerod stuck or lagging?
```

```
P2Pool get_info RPC request to host 157.173.123.245:RPC 18089:ZMQ 18084 failed: Error (empty response), trying again in 1 second
```

P2Pool should switch to a backup node in these case, but if all the backup nodes fails, P2Pool will also fail.

To fix this you can start your own local node by using the [Node](#node) tab.

Running and using your own local Monero node improves privacy and ensures your connection is as stable as your own internet connection. This comes at the cost of downloading and syncing Monero's blockchain yourself (currently 100GB pruned). If you have the disk space, consider using the [Node](#node) tab and connecting to your own Monero node.
See this [issue](https://github.com/gupax-io/gupax/issues/51).

## FAQ


### Where are updates downloaded from?
The latest versions are downloaded using GitHub's API.
* Gupax [`https://github.com/gupax-io/gupax`](https://github.com/gupax-io/gupax)
### Can I quit mid-update?
If you started an update, you should let it finish. If the update has been stuck for a *long* time, quitting Gupax is probably okay. The worst that can happen is that your `Gupax/Node/P2Pool/XMRig/Proxy` binaries may be moved/deleted. Those can be easily redownloaded. Your actual `Gupax` user data (settings, custom nodes, pools, etc) is never touched.

Although Gupax uses a temporary folder (`gupax_update_[A-Za-z0-9]`) to store temporary downloaded files, there aren't measures in place to revert an upgrade once the file swapping has actually started. If you quit Gupax anytime before the `Upgrading packages` phase (after metadata, download, extraction), you will technically be safe but this is not recommended as it is risky, especially since these updates can be very fast.

### Bundled vs Standalone
`Bundled` Gupax comes with the latest version of Node/P2Pool/XMRig/Proxy already in the `zip/tar`.

`Standalone` only contains the Gupax executable.

### How much memory/cpu does Gupax use?
#### Memory
It is using about 10 megabytes of memory on x86_64 

#### CPU
On a Thinkpad T440 with a i5-4300U (2013), it uses about 0.5% of the CPU when unfocused.
When interacted (mouse moving over the window), it will uses about 4.5% CPU (because of [immediate mode](wikipedia.org/wiki/Immediate_mode_(computer_graphics)) which allows a very fast reaction).

### How is sudo handled? (on macOS/Linux)
[See here for more info.](./SUDO.md)

### Why does Gupax need to be Admin? (on Windows)
[See here for more info.](./ADMIN.md)

## License

![GPL v3](assets/images/gplv3-with-text-136x68.png)

[Gupax](./LICENSE), [P2Pool](https://github.com/SChernykh/p2pool/blob/master/LICENSE), [XMRig](https://github.com/xmrig/xmrig/blob/master/LICENSE) and [XMRig-Proxy](https://github.com/xmrig/xmrig-proxy/blob/master/LICENSE) are licensed under the GNU General Public License v3.0.

[Monerod](https://github.com/monero-project/monero) [licence](https://github.com/monero-project/monero?tab=License-1-ov-file)

[See the licenses of various dependencies.](./Cargo.toml)

## Mirror
In case Github repository is down, you can still find the source code at [librejo](https://librejo.monerodevs.org/Ecosystem/gupaxx)

## Donations
If you'd like to thank me for the development of Gupax and/or motivate me to improve it you're welcome to send any amount of XMR to the following address:

![QR CODE DONATION ADDRESS](assets/donation_qr.png)
```
4AGJScWSv45E28pmwck9YRP21KuwGx6fuMYV9kTxXFnWEij5FVEUyccBs7ExDy419DJXRPw3u57TH5BaGbsHTdnf6SvY5p5
```

Every donations will be converted to hours of work !

### Donation transparency

A Kuno page exist so you can easily keep track of the amount funded in this project.  
[Gupax Kuno](https://kuno.anne.media/fundraiser/dsrr/)  
In case you don't want to rely on the kuno website, the secret view key is:  

```
6c6f841e1eda3fba95f2261baa4614e3ec614af2a97176bbae2c0be5281d1d0f
```
