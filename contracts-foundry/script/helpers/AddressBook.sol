// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

import "forge-std/Script.sol";

contract AddressBook is Script {
    function saveDeployment(
        uint256 chainId,
        address guardian,
        address factory,
        address router,
        address htlc,
        address paymaster,
        address entryPoint
    ) internal {
        string memory objectKey = "deployment";
        
        vm.serializeUint(objectKey, "chainId", chainId);
        vm.serializeAddress(objectKey, "guardian", guardian);
        vm.serializeAddress(objectKey, "factory", factory);
        vm.serializeAddress(objectKey, "router", router);
        vm.serializeAddress(objectKey, "htlc", htlc);
        vm.serializeAddress(objectKey, "paymaster", paymaster);
        vm.serializeAddress(objectKey, "entryPoint", entryPoint);
        vm.serializeUint(objectKey, "timestamp", block.timestamp);
        
        // Finalize the JSON object
        string memory json = vm.serializeString(objectKey, "protocol", "yonodex");
        
        // Write to deployments/{chainId}.json
        string memory path = string.concat("./deployments/", vm.toString(chainId), ".json");
        vm.writeJson(json, path);
    }
}