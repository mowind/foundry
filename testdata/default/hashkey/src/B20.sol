// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/// Minimal caller interface for the HashKey B20 native precompiles.
/// Only the functions exercised by the integration fixture are declared.

interface IB20Factory {
    enum B20Variant {
        ASSET,
        STABLECOIN
    }

    struct B20AssetCreateParams {
        uint8 version;
        string name;
        string symbol;
        address initialAdmin;
        uint8 decimals;
    }

    struct B20StablecoinCreateParams {
        uint8 version;
        string name;
        string symbol;
        address initialAdmin;
        string currency;
    }

    function createB20(
        B20Variant variant,
        bytes32 salt,
        bytes calldata params,
        bytes[] calldata initCalls
    ) external returns (address token);

    function getB20Address(B20Variant variant, address sender, bytes32 salt)
        external
        view
        returns (address);

    function isB20(address token) external view returns (bool);

    function isB20Initialized(address token) external view returns (bool);
}

interface IB20 {
    function name() external view returns (string memory);
    function symbol() external view returns (string memory);
    function decimals() external view returns (uint8);
    function totalSupply() external view returns (uint256);
    function balanceOf(address account) external view returns (uint256);
    function transfer(address to, uint256 amount) external returns (bool);
    function mint(address to, uint256 amount) external;
    function currency() external view returns (string memory);
    function MINT_ROLE() external view returns (bytes32);
    function DEFAULT_ADMIN_ROLE() external view returns (bytes32);
    function hasRole(bytes32 role, address account) external view returns (bool);
    function grantRole(bytes32 role, address account) external;
}

interface IActivationRegistry {
    /// Caller is not authorized to activate/deactivate features.
    error Unauthorized(address caller);
    /// Feature is already activated.
    error AlreadyActivated(bytes32 feature);
    /// Feature is not activated.
    error FeatureNotActivated(bytes32 feature);

    function isActivated(bytes32 feature) external view returns (bool);
    function checkActivated(bytes32 feature) external view;
    function admin() external view returns (address);
    function activate(bytes32 feature) external;
    function deactivate(bytes32 feature) external;
}

interface IPolicyRegistry {
    enum PolicyType {
        BLOCKLIST,
        ALLOWLIST
    }

    error Unauthorized();
    error PolicyNotFound();
    error ZeroAddress();

    function createPolicy(address admin, PolicyType policyType) external returns (uint64);
    function createPolicyWithAccounts(address admin, PolicyType policyType, address[] calldata accounts)
        external
        returns (uint64);
    function isAuthorized(uint64 policyId, address account) external view returns (bool);
    function policyExists(uint64 policyId) external view returns (bool);
    function policyAdmin(uint64 policyId) external view returns (address);
    function updateBlocklist(uint64 policyId, bool blocked, address[] calldata accounts) external;
    function updateAllowlist(uint64 policyId, bool allowed, address[] calldata accounts) external;
}

/// Helper that encodes B20 create params for the factory call.
/// Keeping this in Solidity ensures the ABI encoding matches the native precompile exactly.
contract B20Caller {
    address constant FACTORY = 0xB20f000000000000000000000000000000000000;

    function encodeAssetParams(
        string memory name,
        string memory symbol,
        address admin,
        uint8 decimals
    ) internal pure returns (bytes memory) {
        return abi.encode(
            IB20Factory.B20AssetCreateParams({
                version: 1,
                name: name,
                symbol: symbol,
                initialAdmin: admin,
                decimals: decimals
            })
        );
    }

    function encodeStablecoinParams(
        string memory name,
        string memory symbol,
        address admin,
        string memory currency
    ) internal pure returns (bytes memory) {
        return abi.encode(
            IB20Factory.B20StablecoinCreateParams({
                version: 1,
                name: name,
                symbol: symbol,
                initialAdmin: admin,
                currency: currency
            })
        );
    }

    function createAsset(bytes32 salt, string memory name, string memory symbol, address admin)
        external
        returns (address token)
    {
        bytes memory params = encodeAssetParams(name, symbol, admin, 18);
        bytes[] memory noInitCalls = new bytes[](0);
        token = IB20Factory(FACTORY).createB20(IB20Factory.B20Variant.ASSET, salt, params, noInitCalls);
    }

    function createAssetWithInitCalls(
        bytes32 salt,
        string memory name,
        string memory symbol,
        address admin,
        bytes[] memory initCalls
    ) external returns (address token) {
        bytes memory params = encodeAssetParams(name, symbol, admin, 18);
        token = IB20Factory(FACTORY).createB20(IB20Factory.B20Variant.ASSET, salt, params, initCalls);
    }

    function createStablecoin(
        bytes32 salt,
        string memory name,
        string memory symbol,
        address admin,
        string memory currency
    ) external returns (address token) {
        bytes memory params = encodeStablecoinParams(name, symbol, admin, currency);
        bytes[] memory noInitCalls = new bytes[](0);
        token =
            IB20Factory(FACTORY).createB20(IB20Factory.B20Variant.STABLECOIN, salt, params, noInitCalls);
    }

    function predictAddress(IB20Factory.B20Variant variant, address creator, bytes32 salt)
        external
        view
        returns (address)
    {
        return IB20Factory(FACTORY).getB20Address(variant, creator, salt);
    }

    function isInitialized(address token) external view returns (bool) {
        return IB20Factory(FACTORY).isB20Initialized(token);
    }
}
