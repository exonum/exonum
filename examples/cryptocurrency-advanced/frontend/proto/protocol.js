/*eslint-disable block-scoped-var, id-length, no-control-regex, no-magic-numbers, no-prototype-builtins, no-redeclare, no-shadow, no-var, sort-vars*/
(function(global, factory) { /* global define, require, module */

    /* AMD */ if (typeof define === 'function' && define.amd)
        define(["protobufjs/minimal"], factory);

    /* CommonJS */ else if (typeof require === 'function' && typeof module === 'object' && module && module.exports)
        module.exports = factory(require("protobufjs/minimal"));

})(this, function($protobuf) {
    "use strict";

    // Common aliases
    var $Reader = $protobuf.Reader, $Writer = $protobuf.Writer, $util = $protobuf.util;
    
    // Exported root namespace
    var $root = $protobuf.roots["default"] || ($protobuf.roots["default"] = {});
    
    $root.exonum = (function() {
    
        /**
         * Namespace exonum.
         * @exports exonum
         * @namespace
         */
        var exonum = {};
    
        exonum.consensus = (function() {
    
            /**
             * Namespace consensus.
             * @memberof exonum
             * @namespace
             */
            var consensus = {};
    
            consensus.Connect = (function() {
    
                /**
                 * Properties of a Connect.
                 * @memberof exonum.consensus
                 * @interface IConnect
                 * @property {string|null} [pubAddr] Connect pubAddr
                 * @property {google.protobuf.ITimestamp|null} [time] Connect time
                 * @property {string|null} [userAgent] Connect userAgent
                 */
    
                /**
                 * Constructs a new Connect.
                 * @memberof exonum.consensus
                 * @classdesc Represents a Connect.
                 * @implements IConnect
                 * @constructor
                 * @param {exonum.consensus.IConnect=} [properties] Properties to set
                 */
                function Connect(properties) {
                    if (properties)
                        for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                            if (properties[keys[i]] != null)
                                this[keys[i]] = properties[keys[i]];
                }
    
                /**
                 * Connect pubAddr.
                 * @member {string} pubAddr
                 * @memberof exonum.consensus.Connect
                 * @instance
                 */
                Connect.prototype.pubAddr = "";
    
                /**
                 * Connect time.
                 * @member {google.protobuf.ITimestamp|null|undefined} time
                 * @memberof exonum.consensus.Connect
                 * @instance
                 */
                Connect.prototype.time = null;
    
                /**
                 * Connect userAgent.
                 * @member {string} userAgent
                 * @memberof exonum.consensus.Connect
                 * @instance
                 */
                Connect.prototype.userAgent = "";
    
                /**
                 * Creates a new Connect instance using the specified properties.
                 * @function create
                 * @memberof exonum.consensus.Connect
                 * @static
                 * @param {exonum.consensus.IConnect=} [properties] Properties to set
                 * @returns {exonum.consensus.Connect} Connect instance
                 */
                Connect.create = function create(properties) {
                    return new Connect(properties);
                };
    
                /**
                 * Encodes the specified Connect message. Does not implicitly {@link exonum.consensus.Connect.verify|verify} messages.
                 * @function encode
                 * @memberof exonum.consensus.Connect
                 * @static
                 * @param {exonum.consensus.IConnect} message Connect message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                Connect.encode = function encode(message, writer) {
                    if (!writer)
                        writer = $Writer.create();
                    if (message.pubAddr != null && message.hasOwnProperty("pubAddr"))
                        writer.uint32(/* id 1, wireType 2 =*/10).string(message.pubAddr);
                    if (message.time != null && message.hasOwnProperty("time"))
                        $root.google.protobuf.Timestamp.encode(message.time, writer.uint32(/* id 2, wireType 2 =*/18).fork()).ldelim();
                    if (message.userAgent != null && message.hasOwnProperty("userAgent"))
                        writer.uint32(/* id 3, wireType 2 =*/26).string(message.userAgent);
                    return writer;
                };
    
                /**
                 * Encodes the specified Connect message, length delimited. Does not implicitly {@link exonum.consensus.Connect.verify|verify} messages.
                 * @function encodeDelimited
                 * @memberof exonum.consensus.Connect
                 * @static
                 * @param {exonum.consensus.IConnect} message Connect message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                Connect.encodeDelimited = function encodeDelimited(message, writer) {
                    return this.encode(message, writer).ldelim();
                };
    
                /**
                 * Decodes a Connect message from the specified reader or buffer.
                 * @function decode
                 * @memberof exonum.consensus.Connect
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @param {number} [length] Message length if known beforehand
                 * @returns {exonum.consensus.Connect} Connect
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                Connect.decode = function decode(reader, length) {
                    if (!(reader instanceof $Reader))
                        reader = $Reader.create(reader);
                    var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.consensus.Connect();
                    while (reader.pos < end) {
                        var tag = reader.uint32();
                        switch (tag >>> 3) {
                        case 1:
                            message.pubAddr = reader.string();
                            break;
                        case 2:
                            message.time = $root.google.protobuf.Timestamp.decode(reader, reader.uint32());
                            break;
                        case 3:
                            message.userAgent = reader.string();
                            break;
                        default:
                            reader.skipType(tag & 7);
                            break;
                        }
                    }
                    return message;
                };
    
                /**
                 * Decodes a Connect message from the specified reader or buffer, length delimited.
                 * @function decodeDelimited
                 * @memberof exonum.consensus.Connect
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @returns {exonum.consensus.Connect} Connect
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                Connect.decodeDelimited = function decodeDelimited(reader) {
                    if (!(reader instanceof $Reader))
                        reader = new $Reader(reader);
                    return this.decode(reader, reader.uint32());
                };
    
                /**
                 * Verifies a Connect message.
                 * @function verify
                 * @memberof exonum.consensus.Connect
                 * @static
                 * @param {Object.<string,*>} message Plain object to verify
                 * @returns {string|null} `null` if valid, otherwise the reason why it is not
                 */
                Connect.verify = function verify(message) {
                    if (typeof message !== "object" || message === null)
                        return "object expected";
                    if (message.pubAddr != null && message.hasOwnProperty("pubAddr"))
                        if (!$util.isString(message.pubAddr))
                            return "pubAddr: string expected";
                    if (message.time != null && message.hasOwnProperty("time")) {
                        var error = $root.google.protobuf.Timestamp.verify(message.time);
                        if (error)
                            return "time." + error;
                    }
                    if (message.userAgent != null && message.hasOwnProperty("userAgent"))
                        if (!$util.isString(message.userAgent))
                            return "userAgent: string expected";
                    return null;
                };
    
                /**
                 * Creates a Connect message from a plain object. Also converts values to their respective internal types.
                 * @function fromObject
                 * @memberof exonum.consensus.Connect
                 * @static
                 * @param {Object.<string,*>} object Plain object
                 * @returns {exonum.consensus.Connect} Connect
                 */
                Connect.fromObject = function fromObject(object) {
                    if (object instanceof $root.exonum.consensus.Connect)
                        return object;
                    var message = new $root.exonum.consensus.Connect();
                    if (object.pubAddr != null)
                        message.pubAddr = String(object.pubAddr);
                    if (object.time != null) {
                        if (typeof object.time !== "object")
                            throw TypeError(".exonum.consensus.Connect.time: object expected");
                        message.time = $root.google.protobuf.Timestamp.fromObject(object.time);
                    }
                    if (object.userAgent != null)
                        message.userAgent = String(object.userAgent);
                    return message;
                };
    
                /**
                 * Creates a plain object from a Connect message. Also converts values to other types if specified.
                 * @function toObject
                 * @memberof exonum.consensus.Connect
                 * @static
                 * @param {exonum.consensus.Connect} message Connect
                 * @param {$protobuf.IConversionOptions} [options] Conversion options
                 * @returns {Object.<string,*>} Plain object
                 */
                Connect.toObject = function toObject(message, options) {
                    if (!options)
                        options = {};
                    var object = {};
                    if (options.defaults) {
                        object.pubAddr = "";
                        object.time = null;
                        object.userAgent = "";
                    }
                    if (message.pubAddr != null && message.hasOwnProperty("pubAddr"))
                        object.pubAddr = message.pubAddr;
                    if (message.time != null && message.hasOwnProperty("time"))
                        object.time = $root.google.protobuf.Timestamp.toObject(message.time, options);
                    if (message.userAgent != null && message.hasOwnProperty("userAgent"))
                        object.userAgent = message.userAgent;
                    return object;
                };
    
                /**
                 * Converts this Connect to JSON.
                 * @function toJSON
                 * @memberof exonum.consensus.Connect
                 * @instance
                 * @returns {Object.<string,*>} JSON object
                 */
                Connect.prototype.toJSON = function toJSON() {
                    return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                };
    
                return Connect;
            })();
    
            consensus.Status = (function() {
    
                /**
                 * Properties of a Status.
                 * @memberof exonum.consensus
                 * @interface IStatus
                 * @property {number|Long|null} [height] Status height
                 * @property {exonum.IHash|null} [lastHash] Status lastHash
                 */
    
                /**
                 * Constructs a new Status.
                 * @memberof exonum.consensus
                 * @classdesc Represents a Status.
                 * @implements IStatus
                 * @constructor
                 * @param {exonum.consensus.IStatus=} [properties] Properties to set
                 */
                function Status(properties) {
                    if (properties)
                        for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                            if (properties[keys[i]] != null)
                                this[keys[i]] = properties[keys[i]];
                }
    
                /**
                 * Status height.
                 * @member {number|Long} height
                 * @memberof exonum.consensus.Status
                 * @instance
                 */
                Status.prototype.height = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
                /**
                 * Status lastHash.
                 * @member {exonum.IHash|null|undefined} lastHash
                 * @memberof exonum.consensus.Status
                 * @instance
                 */
                Status.prototype.lastHash = null;
    
                /**
                 * Creates a new Status instance using the specified properties.
                 * @function create
                 * @memberof exonum.consensus.Status
                 * @static
                 * @param {exonum.consensus.IStatus=} [properties] Properties to set
                 * @returns {exonum.consensus.Status} Status instance
                 */
                Status.create = function create(properties) {
                    return new Status(properties);
                };
    
                /**
                 * Encodes the specified Status message. Does not implicitly {@link exonum.consensus.Status.verify|verify} messages.
                 * @function encode
                 * @memberof exonum.consensus.Status
                 * @static
                 * @param {exonum.consensus.IStatus} message Status message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                Status.encode = function encode(message, writer) {
                    if (!writer)
                        writer = $Writer.create();
                    if (message.height != null && message.hasOwnProperty("height"))
                        writer.uint32(/* id 1, wireType 0 =*/8).uint64(message.height);
                    if (message.lastHash != null && message.hasOwnProperty("lastHash"))
                        $root.exonum.Hash.encode(message.lastHash, writer.uint32(/* id 2, wireType 2 =*/18).fork()).ldelim();
                    return writer;
                };
    
                /**
                 * Encodes the specified Status message, length delimited. Does not implicitly {@link exonum.consensus.Status.verify|verify} messages.
                 * @function encodeDelimited
                 * @memberof exonum.consensus.Status
                 * @static
                 * @param {exonum.consensus.IStatus} message Status message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                Status.encodeDelimited = function encodeDelimited(message, writer) {
                    return this.encode(message, writer).ldelim();
                };
    
                /**
                 * Decodes a Status message from the specified reader or buffer.
                 * @function decode
                 * @memberof exonum.consensus.Status
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @param {number} [length] Message length if known beforehand
                 * @returns {exonum.consensus.Status} Status
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                Status.decode = function decode(reader, length) {
                    if (!(reader instanceof $Reader))
                        reader = $Reader.create(reader);
                    var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.consensus.Status();
                    while (reader.pos < end) {
                        var tag = reader.uint32();
                        switch (tag >>> 3) {
                        case 1:
                            message.height = reader.uint64();
                            break;
                        case 2:
                            message.lastHash = $root.exonum.Hash.decode(reader, reader.uint32());
                            break;
                        default:
                            reader.skipType(tag & 7);
                            break;
                        }
                    }
                    return message;
                };
    
                /**
                 * Decodes a Status message from the specified reader or buffer, length delimited.
                 * @function decodeDelimited
                 * @memberof exonum.consensus.Status
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @returns {exonum.consensus.Status} Status
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                Status.decodeDelimited = function decodeDelimited(reader) {
                    if (!(reader instanceof $Reader))
                        reader = new $Reader(reader);
                    return this.decode(reader, reader.uint32());
                };
    
                /**
                 * Verifies a Status message.
                 * @function verify
                 * @memberof exonum.consensus.Status
                 * @static
                 * @param {Object.<string,*>} message Plain object to verify
                 * @returns {string|null} `null` if valid, otherwise the reason why it is not
                 */
                Status.verify = function verify(message) {
                    if (typeof message !== "object" || message === null)
                        return "object expected";
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (!$util.isInteger(message.height) && !(message.height && $util.isInteger(message.height.low) && $util.isInteger(message.height.high)))
                            return "height: integer|Long expected";
                    if (message.lastHash != null && message.hasOwnProperty("lastHash")) {
                        var error = $root.exonum.Hash.verify(message.lastHash);
                        if (error)
                            return "lastHash." + error;
                    }
                    return null;
                };
    
                /**
                 * Creates a Status message from a plain object. Also converts values to their respective internal types.
                 * @function fromObject
                 * @memberof exonum.consensus.Status
                 * @static
                 * @param {Object.<string,*>} object Plain object
                 * @returns {exonum.consensus.Status} Status
                 */
                Status.fromObject = function fromObject(object) {
                    if (object instanceof $root.exonum.consensus.Status)
                        return object;
                    var message = new $root.exonum.consensus.Status();
                    if (object.height != null)
                        if ($util.Long)
                            (message.height = $util.Long.fromValue(object.height)).unsigned = true;
                        else if (typeof object.height === "string")
                            message.height = parseInt(object.height, 10);
                        else if (typeof object.height === "number")
                            message.height = object.height;
                        else if (typeof object.height === "object")
                            message.height = new $util.LongBits(object.height.low >>> 0, object.height.high >>> 0).toNumber(true);
                    if (object.lastHash != null) {
                        if (typeof object.lastHash !== "object")
                            throw TypeError(".exonum.consensus.Status.lastHash: object expected");
                        message.lastHash = $root.exonum.Hash.fromObject(object.lastHash);
                    }
                    return message;
                };
    
                /**
                 * Creates a plain object from a Status message. Also converts values to other types if specified.
                 * @function toObject
                 * @memberof exonum.consensus.Status
                 * @static
                 * @param {exonum.consensus.Status} message Status
                 * @param {$protobuf.IConversionOptions} [options] Conversion options
                 * @returns {Object.<string,*>} Plain object
                 */
                Status.toObject = function toObject(message, options) {
                    if (!options)
                        options = {};
                    var object = {};
                    if (options.defaults) {
                        if ($util.Long) {
                            var long = new $util.Long(0, 0, true);
                            object.height = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                        } else
                            object.height = options.longs === String ? "0" : 0;
                        object.lastHash = null;
                    }
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (typeof message.height === "number")
                            object.height = options.longs === String ? String(message.height) : message.height;
                        else
                            object.height = options.longs === String ? $util.Long.prototype.toString.call(message.height) : options.longs === Number ? new $util.LongBits(message.height.low >>> 0, message.height.high >>> 0).toNumber(true) : message.height;
                    if (message.lastHash != null && message.hasOwnProperty("lastHash"))
                        object.lastHash = $root.exonum.Hash.toObject(message.lastHash, options);
                    return object;
                };
    
                /**
                 * Converts this Status to JSON.
                 * @function toJSON
                 * @memberof exonum.consensus.Status
                 * @instance
                 * @returns {Object.<string,*>} JSON object
                 */
                Status.prototype.toJSON = function toJSON() {
                    return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                };
    
                return Status;
            })();
    
            consensus.Propose = (function() {
    
                /**
                 * Properties of a Propose.
                 * @memberof exonum.consensus
                 * @interface IPropose
                 * @property {number|null} [validator] Propose validator
                 * @property {number|Long|null} [height] Propose height
                 * @property {number|null} [round] Propose round
                 * @property {exonum.IHash|null} [prevHash] Propose prevHash
                 * @property {Array.<exonum.IHash>|null} [transactions] Propose transactions
                 */
    
                /**
                 * Constructs a new Propose.
                 * @memberof exonum.consensus
                 * @classdesc Represents a Propose.
                 * @implements IPropose
                 * @constructor
                 * @param {exonum.consensus.IPropose=} [properties] Properties to set
                 */
                function Propose(properties) {
                    this.transactions = [];
                    if (properties)
                        for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                            if (properties[keys[i]] != null)
                                this[keys[i]] = properties[keys[i]];
                }
    
                /**
                 * Propose validator.
                 * @member {number} validator
                 * @memberof exonum.consensus.Propose
                 * @instance
                 */
                Propose.prototype.validator = 0;
    
                /**
                 * Propose height.
                 * @member {number|Long} height
                 * @memberof exonum.consensus.Propose
                 * @instance
                 */
                Propose.prototype.height = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
                /**
                 * Propose round.
                 * @member {number} round
                 * @memberof exonum.consensus.Propose
                 * @instance
                 */
                Propose.prototype.round = 0;
    
                /**
                 * Propose prevHash.
                 * @member {exonum.IHash|null|undefined} prevHash
                 * @memberof exonum.consensus.Propose
                 * @instance
                 */
                Propose.prototype.prevHash = null;
    
                /**
                 * Propose transactions.
                 * @member {Array.<exonum.IHash>} transactions
                 * @memberof exonum.consensus.Propose
                 * @instance
                 */
                Propose.prototype.transactions = $util.emptyArray;
    
                /**
                 * Creates a new Propose instance using the specified properties.
                 * @function create
                 * @memberof exonum.consensus.Propose
                 * @static
                 * @param {exonum.consensus.IPropose=} [properties] Properties to set
                 * @returns {exonum.consensus.Propose} Propose instance
                 */
                Propose.create = function create(properties) {
                    return new Propose(properties);
                };
    
                /**
                 * Encodes the specified Propose message. Does not implicitly {@link exonum.consensus.Propose.verify|verify} messages.
                 * @function encode
                 * @memberof exonum.consensus.Propose
                 * @static
                 * @param {exonum.consensus.IPropose} message Propose message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                Propose.encode = function encode(message, writer) {
                    if (!writer)
                        writer = $Writer.create();
                    if (message.validator != null && message.hasOwnProperty("validator"))
                        writer.uint32(/* id 1, wireType 0 =*/8).uint32(message.validator);
                    if (message.height != null && message.hasOwnProperty("height"))
                        writer.uint32(/* id 2, wireType 0 =*/16).uint64(message.height);
                    if (message.round != null && message.hasOwnProperty("round"))
                        writer.uint32(/* id 3, wireType 0 =*/24).uint32(message.round);
                    if (message.prevHash != null && message.hasOwnProperty("prevHash"))
                        $root.exonum.Hash.encode(message.prevHash, writer.uint32(/* id 4, wireType 2 =*/34).fork()).ldelim();
                    if (message.transactions != null && message.transactions.length)
                        for (var i = 0; i < message.transactions.length; ++i)
                            $root.exonum.Hash.encode(message.transactions[i], writer.uint32(/* id 5, wireType 2 =*/42).fork()).ldelim();
                    return writer;
                };
    
                /**
                 * Encodes the specified Propose message, length delimited. Does not implicitly {@link exonum.consensus.Propose.verify|verify} messages.
                 * @function encodeDelimited
                 * @memberof exonum.consensus.Propose
                 * @static
                 * @param {exonum.consensus.IPropose} message Propose message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                Propose.encodeDelimited = function encodeDelimited(message, writer) {
                    return this.encode(message, writer).ldelim();
                };
    
                /**
                 * Decodes a Propose message from the specified reader or buffer.
                 * @function decode
                 * @memberof exonum.consensus.Propose
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @param {number} [length] Message length if known beforehand
                 * @returns {exonum.consensus.Propose} Propose
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                Propose.decode = function decode(reader, length) {
                    if (!(reader instanceof $Reader))
                        reader = $Reader.create(reader);
                    var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.consensus.Propose();
                    while (reader.pos < end) {
                        var tag = reader.uint32();
                        switch (tag >>> 3) {
                        case 1:
                            message.validator = reader.uint32();
                            break;
                        case 2:
                            message.height = reader.uint64();
                            break;
                        case 3:
                            message.round = reader.uint32();
                            break;
                        case 4:
                            message.prevHash = $root.exonum.Hash.decode(reader, reader.uint32());
                            break;
                        case 5:
                            if (!(message.transactions && message.transactions.length))
                                message.transactions = [];
                            message.transactions.push($root.exonum.Hash.decode(reader, reader.uint32()));
                            break;
                        default:
                            reader.skipType(tag & 7);
                            break;
                        }
                    }
                    return message;
                };
    
                /**
                 * Decodes a Propose message from the specified reader or buffer, length delimited.
                 * @function decodeDelimited
                 * @memberof exonum.consensus.Propose
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @returns {exonum.consensus.Propose} Propose
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                Propose.decodeDelimited = function decodeDelimited(reader) {
                    if (!(reader instanceof $Reader))
                        reader = new $Reader(reader);
                    return this.decode(reader, reader.uint32());
                };
    
                /**
                 * Verifies a Propose message.
                 * @function verify
                 * @memberof exonum.consensus.Propose
                 * @static
                 * @param {Object.<string,*>} message Plain object to verify
                 * @returns {string|null} `null` if valid, otherwise the reason why it is not
                 */
                Propose.verify = function verify(message) {
                    if (typeof message !== "object" || message === null)
                        return "object expected";
                    if (message.validator != null && message.hasOwnProperty("validator"))
                        if (!$util.isInteger(message.validator))
                            return "validator: integer expected";
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (!$util.isInteger(message.height) && !(message.height && $util.isInteger(message.height.low) && $util.isInteger(message.height.high)))
                            return "height: integer|Long expected";
                    if (message.round != null && message.hasOwnProperty("round"))
                        if (!$util.isInteger(message.round))
                            return "round: integer expected";
                    if (message.prevHash != null && message.hasOwnProperty("prevHash")) {
                        var error = $root.exonum.Hash.verify(message.prevHash);
                        if (error)
                            return "prevHash." + error;
                    }
                    if (message.transactions != null && message.hasOwnProperty("transactions")) {
                        if (!Array.isArray(message.transactions))
                            return "transactions: array expected";
                        for (var i = 0; i < message.transactions.length; ++i) {
                            var error = $root.exonum.Hash.verify(message.transactions[i]);
                            if (error)
                                return "transactions." + error;
                        }
                    }
                    return null;
                };
    
                /**
                 * Creates a Propose message from a plain object. Also converts values to their respective internal types.
                 * @function fromObject
                 * @memberof exonum.consensus.Propose
                 * @static
                 * @param {Object.<string,*>} object Plain object
                 * @returns {exonum.consensus.Propose} Propose
                 */
                Propose.fromObject = function fromObject(object) {
                    if (object instanceof $root.exonum.consensus.Propose)
                        return object;
                    var message = new $root.exonum.consensus.Propose();
                    if (object.validator != null)
                        message.validator = object.validator >>> 0;
                    if (object.height != null)
                        if ($util.Long)
                            (message.height = $util.Long.fromValue(object.height)).unsigned = true;
                        else if (typeof object.height === "string")
                            message.height = parseInt(object.height, 10);
                        else if (typeof object.height === "number")
                            message.height = object.height;
                        else if (typeof object.height === "object")
                            message.height = new $util.LongBits(object.height.low >>> 0, object.height.high >>> 0).toNumber(true);
                    if (object.round != null)
                        message.round = object.round >>> 0;
                    if (object.prevHash != null) {
                        if (typeof object.prevHash !== "object")
                            throw TypeError(".exonum.consensus.Propose.prevHash: object expected");
                        message.prevHash = $root.exonum.Hash.fromObject(object.prevHash);
                    }
                    if (object.transactions) {
                        if (!Array.isArray(object.transactions))
                            throw TypeError(".exonum.consensus.Propose.transactions: array expected");
                        message.transactions = [];
                        for (var i = 0; i < object.transactions.length; ++i) {
                            if (typeof object.transactions[i] !== "object")
                                throw TypeError(".exonum.consensus.Propose.transactions: object expected");
                            message.transactions[i] = $root.exonum.Hash.fromObject(object.transactions[i]);
                        }
                    }
                    return message;
                };
    
                /**
                 * Creates a plain object from a Propose message. Also converts values to other types if specified.
                 * @function toObject
                 * @memberof exonum.consensus.Propose
                 * @static
                 * @param {exonum.consensus.Propose} message Propose
                 * @param {$protobuf.IConversionOptions} [options] Conversion options
                 * @returns {Object.<string,*>} Plain object
                 */
                Propose.toObject = function toObject(message, options) {
                    if (!options)
                        options = {};
                    var object = {};
                    if (options.arrays || options.defaults)
                        object.transactions = [];
                    if (options.defaults) {
                        object.validator = 0;
                        if ($util.Long) {
                            var long = new $util.Long(0, 0, true);
                            object.height = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                        } else
                            object.height = options.longs === String ? "0" : 0;
                        object.round = 0;
                        object.prevHash = null;
                    }
                    if (message.validator != null && message.hasOwnProperty("validator"))
                        object.validator = message.validator;
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (typeof message.height === "number")
                            object.height = options.longs === String ? String(message.height) : message.height;
                        else
                            object.height = options.longs === String ? $util.Long.prototype.toString.call(message.height) : options.longs === Number ? new $util.LongBits(message.height.low >>> 0, message.height.high >>> 0).toNumber(true) : message.height;
                    if (message.round != null && message.hasOwnProperty("round"))
                        object.round = message.round;
                    if (message.prevHash != null && message.hasOwnProperty("prevHash"))
                        object.prevHash = $root.exonum.Hash.toObject(message.prevHash, options);
                    if (message.transactions && message.transactions.length) {
                        object.transactions = [];
                        for (var j = 0; j < message.transactions.length; ++j)
                            object.transactions[j] = $root.exonum.Hash.toObject(message.transactions[j], options);
                    }
                    return object;
                };
    
                /**
                 * Converts this Propose to JSON.
                 * @function toJSON
                 * @memberof exonum.consensus.Propose
                 * @instance
                 * @returns {Object.<string,*>} JSON object
                 */
                Propose.prototype.toJSON = function toJSON() {
                    return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                };
    
                return Propose;
            })();
    
            consensus.Prevote = (function() {
    
                /**
                 * Properties of a Prevote.
                 * @memberof exonum.consensus
                 * @interface IPrevote
                 * @property {number|null} [validator] Prevote validator
                 * @property {number|Long|null} [height] Prevote height
                 * @property {number|null} [round] Prevote round
                 * @property {exonum.IHash|null} [proposeHash] Prevote proposeHash
                 * @property {number|null} [lockedRound] Prevote lockedRound
                 */
    
                /**
                 * Constructs a new Prevote.
                 * @memberof exonum.consensus
                 * @classdesc Represents a Prevote.
                 * @implements IPrevote
                 * @constructor
                 * @param {exonum.consensus.IPrevote=} [properties] Properties to set
                 */
                function Prevote(properties) {
                    if (properties)
                        for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                            if (properties[keys[i]] != null)
                                this[keys[i]] = properties[keys[i]];
                }
    
                /**
                 * Prevote validator.
                 * @member {number} validator
                 * @memberof exonum.consensus.Prevote
                 * @instance
                 */
                Prevote.prototype.validator = 0;
    
                /**
                 * Prevote height.
                 * @member {number|Long} height
                 * @memberof exonum.consensus.Prevote
                 * @instance
                 */
                Prevote.prototype.height = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
                /**
                 * Prevote round.
                 * @member {number} round
                 * @memberof exonum.consensus.Prevote
                 * @instance
                 */
                Prevote.prototype.round = 0;
    
                /**
                 * Prevote proposeHash.
                 * @member {exonum.IHash|null|undefined} proposeHash
                 * @memberof exonum.consensus.Prevote
                 * @instance
                 */
                Prevote.prototype.proposeHash = null;
    
                /**
                 * Prevote lockedRound.
                 * @member {number} lockedRound
                 * @memberof exonum.consensus.Prevote
                 * @instance
                 */
                Prevote.prototype.lockedRound = 0;
    
                /**
                 * Creates a new Prevote instance using the specified properties.
                 * @function create
                 * @memberof exonum.consensus.Prevote
                 * @static
                 * @param {exonum.consensus.IPrevote=} [properties] Properties to set
                 * @returns {exonum.consensus.Prevote} Prevote instance
                 */
                Prevote.create = function create(properties) {
                    return new Prevote(properties);
                };
    
                /**
                 * Encodes the specified Prevote message. Does not implicitly {@link exonum.consensus.Prevote.verify|verify} messages.
                 * @function encode
                 * @memberof exonum.consensus.Prevote
                 * @static
                 * @param {exonum.consensus.IPrevote} message Prevote message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                Prevote.encode = function encode(message, writer) {
                    if (!writer)
                        writer = $Writer.create();
                    if (message.validator != null && message.hasOwnProperty("validator"))
                        writer.uint32(/* id 1, wireType 0 =*/8).uint32(message.validator);
                    if (message.height != null && message.hasOwnProperty("height"))
                        writer.uint32(/* id 2, wireType 0 =*/16).uint64(message.height);
                    if (message.round != null && message.hasOwnProperty("round"))
                        writer.uint32(/* id 3, wireType 0 =*/24).uint32(message.round);
                    if (message.proposeHash != null && message.hasOwnProperty("proposeHash"))
                        $root.exonum.Hash.encode(message.proposeHash, writer.uint32(/* id 4, wireType 2 =*/34).fork()).ldelim();
                    if (message.lockedRound != null && message.hasOwnProperty("lockedRound"))
                        writer.uint32(/* id 5, wireType 0 =*/40).uint32(message.lockedRound);
                    return writer;
                };
    
                /**
                 * Encodes the specified Prevote message, length delimited. Does not implicitly {@link exonum.consensus.Prevote.verify|verify} messages.
                 * @function encodeDelimited
                 * @memberof exonum.consensus.Prevote
                 * @static
                 * @param {exonum.consensus.IPrevote} message Prevote message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                Prevote.encodeDelimited = function encodeDelimited(message, writer) {
                    return this.encode(message, writer).ldelim();
                };
    
                /**
                 * Decodes a Prevote message from the specified reader or buffer.
                 * @function decode
                 * @memberof exonum.consensus.Prevote
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @param {number} [length] Message length if known beforehand
                 * @returns {exonum.consensus.Prevote} Prevote
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                Prevote.decode = function decode(reader, length) {
                    if (!(reader instanceof $Reader))
                        reader = $Reader.create(reader);
                    var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.consensus.Prevote();
                    while (reader.pos < end) {
                        var tag = reader.uint32();
                        switch (tag >>> 3) {
                        case 1:
                            message.validator = reader.uint32();
                            break;
                        case 2:
                            message.height = reader.uint64();
                            break;
                        case 3:
                            message.round = reader.uint32();
                            break;
                        case 4:
                            message.proposeHash = $root.exonum.Hash.decode(reader, reader.uint32());
                            break;
                        case 5:
                            message.lockedRound = reader.uint32();
                            break;
                        default:
                            reader.skipType(tag & 7);
                            break;
                        }
                    }
                    return message;
                };
    
                /**
                 * Decodes a Prevote message from the specified reader or buffer, length delimited.
                 * @function decodeDelimited
                 * @memberof exonum.consensus.Prevote
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @returns {exonum.consensus.Prevote} Prevote
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                Prevote.decodeDelimited = function decodeDelimited(reader) {
                    if (!(reader instanceof $Reader))
                        reader = new $Reader(reader);
                    return this.decode(reader, reader.uint32());
                };
    
                /**
                 * Verifies a Prevote message.
                 * @function verify
                 * @memberof exonum.consensus.Prevote
                 * @static
                 * @param {Object.<string,*>} message Plain object to verify
                 * @returns {string|null} `null` if valid, otherwise the reason why it is not
                 */
                Prevote.verify = function verify(message) {
                    if (typeof message !== "object" || message === null)
                        return "object expected";
                    if (message.validator != null && message.hasOwnProperty("validator"))
                        if (!$util.isInteger(message.validator))
                            return "validator: integer expected";
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (!$util.isInteger(message.height) && !(message.height && $util.isInteger(message.height.low) && $util.isInteger(message.height.high)))
                            return "height: integer|Long expected";
                    if (message.round != null && message.hasOwnProperty("round"))
                        if (!$util.isInteger(message.round))
                            return "round: integer expected";
                    if (message.proposeHash != null && message.hasOwnProperty("proposeHash")) {
                        var error = $root.exonum.Hash.verify(message.proposeHash);
                        if (error)
                            return "proposeHash." + error;
                    }
                    if (message.lockedRound != null && message.hasOwnProperty("lockedRound"))
                        if (!$util.isInteger(message.lockedRound))
                            return "lockedRound: integer expected";
                    return null;
                };
    
                /**
                 * Creates a Prevote message from a plain object. Also converts values to their respective internal types.
                 * @function fromObject
                 * @memberof exonum.consensus.Prevote
                 * @static
                 * @param {Object.<string,*>} object Plain object
                 * @returns {exonum.consensus.Prevote} Prevote
                 */
                Prevote.fromObject = function fromObject(object) {
                    if (object instanceof $root.exonum.consensus.Prevote)
                        return object;
                    var message = new $root.exonum.consensus.Prevote();
                    if (object.validator != null)
                        message.validator = object.validator >>> 0;
                    if (object.height != null)
                        if ($util.Long)
                            (message.height = $util.Long.fromValue(object.height)).unsigned = true;
                        else if (typeof object.height === "string")
                            message.height = parseInt(object.height, 10);
                        else if (typeof object.height === "number")
                            message.height = object.height;
                        else if (typeof object.height === "object")
                            message.height = new $util.LongBits(object.height.low >>> 0, object.height.high >>> 0).toNumber(true);
                    if (object.round != null)
                        message.round = object.round >>> 0;
                    if (object.proposeHash != null) {
                        if (typeof object.proposeHash !== "object")
                            throw TypeError(".exonum.consensus.Prevote.proposeHash: object expected");
                        message.proposeHash = $root.exonum.Hash.fromObject(object.proposeHash);
                    }
                    if (object.lockedRound != null)
                        message.lockedRound = object.lockedRound >>> 0;
                    return message;
                };
    
                /**
                 * Creates a plain object from a Prevote message. Also converts values to other types if specified.
                 * @function toObject
                 * @memberof exonum.consensus.Prevote
                 * @static
                 * @param {exonum.consensus.Prevote} message Prevote
                 * @param {$protobuf.IConversionOptions} [options] Conversion options
                 * @returns {Object.<string,*>} Plain object
                 */
                Prevote.toObject = function toObject(message, options) {
                    if (!options)
                        options = {};
                    var object = {};
                    if (options.defaults) {
                        object.validator = 0;
                        if ($util.Long) {
                            var long = new $util.Long(0, 0, true);
                            object.height = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                        } else
                            object.height = options.longs === String ? "0" : 0;
                        object.round = 0;
                        object.proposeHash = null;
                        object.lockedRound = 0;
                    }
                    if (message.validator != null && message.hasOwnProperty("validator"))
                        object.validator = message.validator;
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (typeof message.height === "number")
                            object.height = options.longs === String ? String(message.height) : message.height;
                        else
                            object.height = options.longs === String ? $util.Long.prototype.toString.call(message.height) : options.longs === Number ? new $util.LongBits(message.height.low >>> 0, message.height.high >>> 0).toNumber(true) : message.height;
                    if (message.round != null && message.hasOwnProperty("round"))
                        object.round = message.round;
                    if (message.proposeHash != null && message.hasOwnProperty("proposeHash"))
                        object.proposeHash = $root.exonum.Hash.toObject(message.proposeHash, options);
                    if (message.lockedRound != null && message.hasOwnProperty("lockedRound"))
                        object.lockedRound = message.lockedRound;
                    return object;
                };
    
                /**
                 * Converts this Prevote to JSON.
                 * @function toJSON
                 * @memberof exonum.consensus.Prevote
                 * @instance
                 * @returns {Object.<string,*>} JSON object
                 */
                Prevote.prototype.toJSON = function toJSON() {
                    return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                };
    
                return Prevote;
            })();
    
            consensus.Precommit = (function() {
    
                /**
                 * Properties of a Precommit.
                 * @memberof exonum.consensus
                 * @interface IPrecommit
                 * @property {number|null} [validator] Precommit validator
                 * @property {number|Long|null} [height] Precommit height
                 * @property {number|null} [round] Precommit round
                 * @property {exonum.IHash|null} [proposeHash] Precommit proposeHash
                 * @property {exonum.IHash|null} [blockHash] Precommit blockHash
                 * @property {google.protobuf.ITimestamp|null} [time] Precommit time
                 */
    
                /**
                 * Constructs a new Precommit.
                 * @memberof exonum.consensus
                 * @classdesc Represents a Precommit.
                 * @implements IPrecommit
                 * @constructor
                 * @param {exonum.consensus.IPrecommit=} [properties] Properties to set
                 */
                function Precommit(properties) {
                    if (properties)
                        for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                            if (properties[keys[i]] != null)
                                this[keys[i]] = properties[keys[i]];
                }
    
                /**
                 * Precommit validator.
                 * @member {number} validator
                 * @memberof exonum.consensus.Precommit
                 * @instance
                 */
                Precommit.prototype.validator = 0;
    
                /**
                 * Precommit height.
                 * @member {number|Long} height
                 * @memberof exonum.consensus.Precommit
                 * @instance
                 */
                Precommit.prototype.height = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
                /**
                 * Precommit round.
                 * @member {number} round
                 * @memberof exonum.consensus.Precommit
                 * @instance
                 */
                Precommit.prototype.round = 0;
    
                /**
                 * Precommit proposeHash.
                 * @member {exonum.IHash|null|undefined} proposeHash
                 * @memberof exonum.consensus.Precommit
                 * @instance
                 */
                Precommit.prototype.proposeHash = null;
    
                /**
                 * Precommit blockHash.
                 * @member {exonum.IHash|null|undefined} blockHash
                 * @memberof exonum.consensus.Precommit
                 * @instance
                 */
                Precommit.prototype.blockHash = null;
    
                /**
                 * Precommit time.
                 * @member {google.protobuf.ITimestamp|null|undefined} time
                 * @memberof exonum.consensus.Precommit
                 * @instance
                 */
                Precommit.prototype.time = null;
    
                /**
                 * Creates a new Precommit instance using the specified properties.
                 * @function create
                 * @memberof exonum.consensus.Precommit
                 * @static
                 * @param {exonum.consensus.IPrecommit=} [properties] Properties to set
                 * @returns {exonum.consensus.Precommit} Precommit instance
                 */
                Precommit.create = function create(properties) {
                    return new Precommit(properties);
                };
    
                /**
                 * Encodes the specified Precommit message. Does not implicitly {@link exonum.consensus.Precommit.verify|verify} messages.
                 * @function encode
                 * @memberof exonum.consensus.Precommit
                 * @static
                 * @param {exonum.consensus.IPrecommit} message Precommit message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                Precommit.encode = function encode(message, writer) {
                    if (!writer)
                        writer = $Writer.create();
                    if (message.validator != null && message.hasOwnProperty("validator"))
                        writer.uint32(/* id 1, wireType 0 =*/8).uint32(message.validator);
                    if (message.height != null && message.hasOwnProperty("height"))
                        writer.uint32(/* id 2, wireType 0 =*/16).uint64(message.height);
                    if (message.round != null && message.hasOwnProperty("round"))
                        writer.uint32(/* id 3, wireType 0 =*/24).uint32(message.round);
                    if (message.proposeHash != null && message.hasOwnProperty("proposeHash"))
                        $root.exonum.Hash.encode(message.proposeHash, writer.uint32(/* id 4, wireType 2 =*/34).fork()).ldelim();
                    if (message.blockHash != null && message.hasOwnProperty("blockHash"))
                        $root.exonum.Hash.encode(message.blockHash, writer.uint32(/* id 5, wireType 2 =*/42).fork()).ldelim();
                    if (message.time != null && message.hasOwnProperty("time"))
                        $root.google.protobuf.Timestamp.encode(message.time, writer.uint32(/* id 6, wireType 2 =*/50).fork()).ldelim();
                    return writer;
                };
    
                /**
                 * Encodes the specified Precommit message, length delimited. Does not implicitly {@link exonum.consensus.Precommit.verify|verify} messages.
                 * @function encodeDelimited
                 * @memberof exonum.consensus.Precommit
                 * @static
                 * @param {exonum.consensus.IPrecommit} message Precommit message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                Precommit.encodeDelimited = function encodeDelimited(message, writer) {
                    return this.encode(message, writer).ldelim();
                };
    
                /**
                 * Decodes a Precommit message from the specified reader or buffer.
                 * @function decode
                 * @memberof exonum.consensus.Precommit
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @param {number} [length] Message length if known beforehand
                 * @returns {exonum.consensus.Precommit} Precommit
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                Precommit.decode = function decode(reader, length) {
                    if (!(reader instanceof $Reader))
                        reader = $Reader.create(reader);
                    var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.consensus.Precommit();
                    while (reader.pos < end) {
                        var tag = reader.uint32();
                        switch (tag >>> 3) {
                        case 1:
                            message.validator = reader.uint32();
                            break;
                        case 2:
                            message.height = reader.uint64();
                            break;
                        case 3:
                            message.round = reader.uint32();
                            break;
                        case 4:
                            message.proposeHash = $root.exonum.Hash.decode(reader, reader.uint32());
                            break;
                        case 5:
                            message.blockHash = $root.exonum.Hash.decode(reader, reader.uint32());
                            break;
                        case 6:
                            message.time = $root.google.protobuf.Timestamp.decode(reader, reader.uint32());
                            break;
                        default:
                            reader.skipType(tag & 7);
                            break;
                        }
                    }
                    return message;
                };
    
                /**
                 * Decodes a Precommit message from the specified reader or buffer, length delimited.
                 * @function decodeDelimited
                 * @memberof exonum.consensus.Precommit
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @returns {exonum.consensus.Precommit} Precommit
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                Precommit.decodeDelimited = function decodeDelimited(reader) {
                    if (!(reader instanceof $Reader))
                        reader = new $Reader(reader);
                    return this.decode(reader, reader.uint32());
                };
    
                /**
                 * Verifies a Precommit message.
                 * @function verify
                 * @memberof exonum.consensus.Precommit
                 * @static
                 * @param {Object.<string,*>} message Plain object to verify
                 * @returns {string|null} `null` if valid, otherwise the reason why it is not
                 */
                Precommit.verify = function verify(message) {
                    if (typeof message !== "object" || message === null)
                        return "object expected";
                    if (message.validator != null && message.hasOwnProperty("validator"))
                        if (!$util.isInteger(message.validator))
                            return "validator: integer expected";
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (!$util.isInteger(message.height) && !(message.height && $util.isInteger(message.height.low) && $util.isInteger(message.height.high)))
                            return "height: integer|Long expected";
                    if (message.round != null && message.hasOwnProperty("round"))
                        if (!$util.isInteger(message.round))
                            return "round: integer expected";
                    if (message.proposeHash != null && message.hasOwnProperty("proposeHash")) {
                        var error = $root.exonum.Hash.verify(message.proposeHash);
                        if (error)
                            return "proposeHash." + error;
                    }
                    if (message.blockHash != null && message.hasOwnProperty("blockHash")) {
                        var error = $root.exonum.Hash.verify(message.blockHash);
                        if (error)
                            return "blockHash." + error;
                    }
                    if (message.time != null && message.hasOwnProperty("time")) {
                        var error = $root.google.protobuf.Timestamp.verify(message.time);
                        if (error)
                            return "time." + error;
                    }
                    return null;
                };
    
                /**
                 * Creates a Precommit message from a plain object. Also converts values to their respective internal types.
                 * @function fromObject
                 * @memberof exonum.consensus.Precommit
                 * @static
                 * @param {Object.<string,*>} object Plain object
                 * @returns {exonum.consensus.Precommit} Precommit
                 */
                Precommit.fromObject = function fromObject(object) {
                    if (object instanceof $root.exonum.consensus.Precommit)
                        return object;
                    var message = new $root.exonum.consensus.Precommit();
                    if (object.validator != null)
                        message.validator = object.validator >>> 0;
                    if (object.height != null)
                        if ($util.Long)
                            (message.height = $util.Long.fromValue(object.height)).unsigned = true;
                        else if (typeof object.height === "string")
                            message.height = parseInt(object.height, 10);
                        else if (typeof object.height === "number")
                            message.height = object.height;
                        else if (typeof object.height === "object")
                            message.height = new $util.LongBits(object.height.low >>> 0, object.height.high >>> 0).toNumber(true);
                    if (object.round != null)
                        message.round = object.round >>> 0;
                    if (object.proposeHash != null) {
                        if (typeof object.proposeHash !== "object")
                            throw TypeError(".exonum.consensus.Precommit.proposeHash: object expected");
                        message.proposeHash = $root.exonum.Hash.fromObject(object.proposeHash);
                    }
                    if (object.blockHash != null) {
                        if (typeof object.blockHash !== "object")
                            throw TypeError(".exonum.consensus.Precommit.blockHash: object expected");
                        message.blockHash = $root.exonum.Hash.fromObject(object.blockHash);
                    }
                    if (object.time != null) {
                        if (typeof object.time !== "object")
                            throw TypeError(".exonum.consensus.Precommit.time: object expected");
                        message.time = $root.google.protobuf.Timestamp.fromObject(object.time);
                    }
                    return message;
                };
    
                /**
                 * Creates a plain object from a Precommit message. Also converts values to other types if specified.
                 * @function toObject
                 * @memberof exonum.consensus.Precommit
                 * @static
                 * @param {exonum.consensus.Precommit} message Precommit
                 * @param {$protobuf.IConversionOptions} [options] Conversion options
                 * @returns {Object.<string,*>} Plain object
                 */
                Precommit.toObject = function toObject(message, options) {
                    if (!options)
                        options = {};
                    var object = {};
                    if (options.defaults) {
                        object.validator = 0;
                        if ($util.Long) {
                            var long = new $util.Long(0, 0, true);
                            object.height = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                        } else
                            object.height = options.longs === String ? "0" : 0;
                        object.round = 0;
                        object.proposeHash = null;
                        object.blockHash = null;
                        object.time = null;
                    }
                    if (message.validator != null && message.hasOwnProperty("validator"))
                        object.validator = message.validator;
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (typeof message.height === "number")
                            object.height = options.longs === String ? String(message.height) : message.height;
                        else
                            object.height = options.longs === String ? $util.Long.prototype.toString.call(message.height) : options.longs === Number ? new $util.LongBits(message.height.low >>> 0, message.height.high >>> 0).toNumber(true) : message.height;
                    if (message.round != null && message.hasOwnProperty("round"))
                        object.round = message.round;
                    if (message.proposeHash != null && message.hasOwnProperty("proposeHash"))
                        object.proposeHash = $root.exonum.Hash.toObject(message.proposeHash, options);
                    if (message.blockHash != null && message.hasOwnProperty("blockHash"))
                        object.blockHash = $root.exonum.Hash.toObject(message.blockHash, options);
                    if (message.time != null && message.hasOwnProperty("time"))
                        object.time = $root.google.protobuf.Timestamp.toObject(message.time, options);
                    return object;
                };
    
                /**
                 * Converts this Precommit to JSON.
                 * @function toJSON
                 * @memberof exonum.consensus.Precommit
                 * @instance
                 * @returns {Object.<string,*>} JSON object
                 */
                Precommit.prototype.toJSON = function toJSON() {
                    return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                };
    
                return Precommit;
            })();
    
            consensus.BlockResponse = (function() {
    
                /**
                 * Properties of a BlockResponse.
                 * @memberof exonum.consensus
                 * @interface IBlockResponse
                 * @property {exonum.IPublicKey|null} [to] BlockResponse to
                 * @property {exonum.IBlock|null} [block] BlockResponse block
                 * @property {Array.<Uint8Array>|null} [precommits] BlockResponse precommits
                 * @property {Array.<exonum.IHash>|null} [transactions] BlockResponse transactions
                 */
    
                /**
                 * Constructs a new BlockResponse.
                 * @memberof exonum.consensus
                 * @classdesc Represents a BlockResponse.
                 * @implements IBlockResponse
                 * @constructor
                 * @param {exonum.consensus.IBlockResponse=} [properties] Properties to set
                 */
                function BlockResponse(properties) {
                    this.precommits = [];
                    this.transactions = [];
                    if (properties)
                        for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                            if (properties[keys[i]] != null)
                                this[keys[i]] = properties[keys[i]];
                }
    
                /**
                 * BlockResponse to.
                 * @member {exonum.IPublicKey|null|undefined} to
                 * @memberof exonum.consensus.BlockResponse
                 * @instance
                 */
                BlockResponse.prototype.to = null;
    
                /**
                 * BlockResponse block.
                 * @member {exonum.IBlock|null|undefined} block
                 * @memberof exonum.consensus.BlockResponse
                 * @instance
                 */
                BlockResponse.prototype.block = null;
    
                /**
                 * BlockResponse precommits.
                 * @member {Array.<Uint8Array>} precommits
                 * @memberof exonum.consensus.BlockResponse
                 * @instance
                 */
                BlockResponse.prototype.precommits = $util.emptyArray;
    
                /**
                 * BlockResponse transactions.
                 * @member {Array.<exonum.IHash>} transactions
                 * @memberof exonum.consensus.BlockResponse
                 * @instance
                 */
                BlockResponse.prototype.transactions = $util.emptyArray;
    
                /**
                 * Creates a new BlockResponse instance using the specified properties.
                 * @function create
                 * @memberof exonum.consensus.BlockResponse
                 * @static
                 * @param {exonum.consensus.IBlockResponse=} [properties] Properties to set
                 * @returns {exonum.consensus.BlockResponse} BlockResponse instance
                 */
                BlockResponse.create = function create(properties) {
                    return new BlockResponse(properties);
                };
    
                /**
                 * Encodes the specified BlockResponse message. Does not implicitly {@link exonum.consensus.BlockResponse.verify|verify} messages.
                 * @function encode
                 * @memberof exonum.consensus.BlockResponse
                 * @static
                 * @param {exonum.consensus.IBlockResponse} message BlockResponse message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                BlockResponse.encode = function encode(message, writer) {
                    if (!writer)
                        writer = $Writer.create();
                    if (message.to != null && message.hasOwnProperty("to"))
                        $root.exonum.PublicKey.encode(message.to, writer.uint32(/* id 1, wireType 2 =*/10).fork()).ldelim();
                    if (message.block != null && message.hasOwnProperty("block"))
                        $root.exonum.Block.encode(message.block, writer.uint32(/* id 2, wireType 2 =*/18).fork()).ldelim();
                    if (message.precommits != null && message.precommits.length)
                        for (var i = 0; i < message.precommits.length; ++i)
                            writer.uint32(/* id 3, wireType 2 =*/26).bytes(message.precommits[i]);
                    if (message.transactions != null && message.transactions.length)
                        for (var i = 0; i < message.transactions.length; ++i)
                            $root.exonum.Hash.encode(message.transactions[i], writer.uint32(/* id 4, wireType 2 =*/34).fork()).ldelim();
                    return writer;
                };
    
                /**
                 * Encodes the specified BlockResponse message, length delimited. Does not implicitly {@link exonum.consensus.BlockResponse.verify|verify} messages.
                 * @function encodeDelimited
                 * @memberof exonum.consensus.BlockResponse
                 * @static
                 * @param {exonum.consensus.IBlockResponse} message BlockResponse message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                BlockResponse.encodeDelimited = function encodeDelimited(message, writer) {
                    return this.encode(message, writer).ldelim();
                };
    
                /**
                 * Decodes a BlockResponse message from the specified reader or buffer.
                 * @function decode
                 * @memberof exonum.consensus.BlockResponse
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @param {number} [length] Message length if known beforehand
                 * @returns {exonum.consensus.BlockResponse} BlockResponse
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                BlockResponse.decode = function decode(reader, length) {
                    if (!(reader instanceof $Reader))
                        reader = $Reader.create(reader);
                    var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.consensus.BlockResponse();
                    while (reader.pos < end) {
                        var tag = reader.uint32();
                        switch (tag >>> 3) {
                        case 1:
                            message.to = $root.exonum.PublicKey.decode(reader, reader.uint32());
                            break;
                        case 2:
                            message.block = $root.exonum.Block.decode(reader, reader.uint32());
                            break;
                        case 3:
                            if (!(message.precommits && message.precommits.length))
                                message.precommits = [];
                            message.precommits.push(reader.bytes());
                            break;
                        case 4:
                            if (!(message.transactions && message.transactions.length))
                                message.transactions = [];
                            message.transactions.push($root.exonum.Hash.decode(reader, reader.uint32()));
                            break;
                        default:
                            reader.skipType(tag & 7);
                            break;
                        }
                    }
                    return message;
                };
    
                /**
                 * Decodes a BlockResponse message from the specified reader or buffer, length delimited.
                 * @function decodeDelimited
                 * @memberof exonum.consensus.BlockResponse
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @returns {exonum.consensus.BlockResponse} BlockResponse
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                BlockResponse.decodeDelimited = function decodeDelimited(reader) {
                    if (!(reader instanceof $Reader))
                        reader = new $Reader(reader);
                    return this.decode(reader, reader.uint32());
                };
    
                /**
                 * Verifies a BlockResponse message.
                 * @function verify
                 * @memberof exonum.consensus.BlockResponse
                 * @static
                 * @param {Object.<string,*>} message Plain object to verify
                 * @returns {string|null} `null` if valid, otherwise the reason why it is not
                 */
                BlockResponse.verify = function verify(message) {
                    if (typeof message !== "object" || message === null)
                        return "object expected";
                    if (message.to != null && message.hasOwnProperty("to")) {
                        var error = $root.exonum.PublicKey.verify(message.to);
                        if (error)
                            return "to." + error;
                    }
                    if (message.block != null && message.hasOwnProperty("block")) {
                        var error = $root.exonum.Block.verify(message.block);
                        if (error)
                            return "block." + error;
                    }
                    if (message.precommits != null && message.hasOwnProperty("precommits")) {
                        if (!Array.isArray(message.precommits))
                            return "precommits: array expected";
                        for (var i = 0; i < message.precommits.length; ++i)
                            if (!(message.precommits[i] && typeof message.precommits[i].length === "number" || $util.isString(message.precommits[i])))
                                return "precommits: buffer[] expected";
                    }
                    if (message.transactions != null && message.hasOwnProperty("transactions")) {
                        if (!Array.isArray(message.transactions))
                            return "transactions: array expected";
                        for (var i = 0; i < message.transactions.length; ++i) {
                            var error = $root.exonum.Hash.verify(message.transactions[i]);
                            if (error)
                                return "transactions." + error;
                        }
                    }
                    return null;
                };
    
                /**
                 * Creates a BlockResponse message from a plain object. Also converts values to their respective internal types.
                 * @function fromObject
                 * @memberof exonum.consensus.BlockResponse
                 * @static
                 * @param {Object.<string,*>} object Plain object
                 * @returns {exonum.consensus.BlockResponse} BlockResponse
                 */
                BlockResponse.fromObject = function fromObject(object) {
                    if (object instanceof $root.exonum.consensus.BlockResponse)
                        return object;
                    var message = new $root.exonum.consensus.BlockResponse();
                    if (object.to != null) {
                        if (typeof object.to !== "object")
                            throw TypeError(".exonum.consensus.BlockResponse.to: object expected");
                        message.to = $root.exonum.PublicKey.fromObject(object.to);
                    }
                    if (object.block != null) {
                        if (typeof object.block !== "object")
                            throw TypeError(".exonum.consensus.BlockResponse.block: object expected");
                        message.block = $root.exonum.Block.fromObject(object.block);
                    }
                    if (object.precommits) {
                        if (!Array.isArray(object.precommits))
                            throw TypeError(".exonum.consensus.BlockResponse.precommits: array expected");
                        message.precommits = [];
                        for (var i = 0; i < object.precommits.length; ++i)
                            if (typeof object.precommits[i] === "string")
                                $util.base64.decode(object.precommits[i], message.precommits[i] = $util.newBuffer($util.base64.length(object.precommits[i])), 0);
                            else if (object.precommits[i].length)
                                message.precommits[i] = object.precommits[i];
                    }
                    if (object.transactions) {
                        if (!Array.isArray(object.transactions))
                            throw TypeError(".exonum.consensus.BlockResponse.transactions: array expected");
                        message.transactions = [];
                        for (var i = 0; i < object.transactions.length; ++i) {
                            if (typeof object.transactions[i] !== "object")
                                throw TypeError(".exonum.consensus.BlockResponse.transactions: object expected");
                            message.transactions[i] = $root.exonum.Hash.fromObject(object.transactions[i]);
                        }
                    }
                    return message;
                };
    
                /**
                 * Creates a plain object from a BlockResponse message. Also converts values to other types if specified.
                 * @function toObject
                 * @memberof exonum.consensus.BlockResponse
                 * @static
                 * @param {exonum.consensus.BlockResponse} message BlockResponse
                 * @param {$protobuf.IConversionOptions} [options] Conversion options
                 * @returns {Object.<string,*>} Plain object
                 */
                BlockResponse.toObject = function toObject(message, options) {
                    if (!options)
                        options = {};
                    var object = {};
                    if (options.arrays || options.defaults) {
                        object.precommits = [];
                        object.transactions = [];
                    }
                    if (options.defaults) {
                        object.to = null;
                        object.block = null;
                    }
                    if (message.to != null && message.hasOwnProperty("to"))
                        object.to = $root.exonum.PublicKey.toObject(message.to, options);
                    if (message.block != null && message.hasOwnProperty("block"))
                        object.block = $root.exonum.Block.toObject(message.block, options);
                    if (message.precommits && message.precommits.length) {
                        object.precommits = [];
                        for (var j = 0; j < message.precommits.length; ++j)
                            object.precommits[j] = options.bytes === String ? $util.base64.encode(message.precommits[j], 0, message.precommits[j].length) : options.bytes === Array ? Array.prototype.slice.call(message.precommits[j]) : message.precommits[j];
                    }
                    if (message.transactions && message.transactions.length) {
                        object.transactions = [];
                        for (var j = 0; j < message.transactions.length; ++j)
                            object.transactions[j] = $root.exonum.Hash.toObject(message.transactions[j], options);
                    }
                    return object;
                };
    
                /**
                 * Converts this BlockResponse to JSON.
                 * @function toJSON
                 * @memberof exonum.consensus.BlockResponse
                 * @instance
                 * @returns {Object.<string,*>} JSON object
                 */
                BlockResponse.prototype.toJSON = function toJSON() {
                    return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                };
    
                return BlockResponse;
            })();
    
            consensus.TransactionsResponse = (function() {
    
                /**
                 * Properties of a TransactionsResponse.
                 * @memberof exonum.consensus
                 * @interface ITransactionsResponse
                 * @property {exonum.IPublicKey|null} [to] TransactionsResponse to
                 * @property {Array.<Uint8Array>|null} [transactions] TransactionsResponse transactions
                 */
    
                /**
                 * Constructs a new TransactionsResponse.
                 * @memberof exonum.consensus
                 * @classdesc Represents a TransactionsResponse.
                 * @implements ITransactionsResponse
                 * @constructor
                 * @param {exonum.consensus.ITransactionsResponse=} [properties] Properties to set
                 */
                function TransactionsResponse(properties) {
                    this.transactions = [];
                    if (properties)
                        for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                            if (properties[keys[i]] != null)
                                this[keys[i]] = properties[keys[i]];
                }
    
                /**
                 * TransactionsResponse to.
                 * @member {exonum.IPublicKey|null|undefined} to
                 * @memberof exonum.consensus.TransactionsResponse
                 * @instance
                 */
                TransactionsResponse.prototype.to = null;
    
                /**
                 * TransactionsResponse transactions.
                 * @member {Array.<Uint8Array>} transactions
                 * @memberof exonum.consensus.TransactionsResponse
                 * @instance
                 */
                TransactionsResponse.prototype.transactions = $util.emptyArray;
    
                /**
                 * Creates a new TransactionsResponse instance using the specified properties.
                 * @function create
                 * @memberof exonum.consensus.TransactionsResponse
                 * @static
                 * @param {exonum.consensus.ITransactionsResponse=} [properties] Properties to set
                 * @returns {exonum.consensus.TransactionsResponse} TransactionsResponse instance
                 */
                TransactionsResponse.create = function create(properties) {
                    return new TransactionsResponse(properties);
                };
    
                /**
                 * Encodes the specified TransactionsResponse message. Does not implicitly {@link exonum.consensus.TransactionsResponse.verify|verify} messages.
                 * @function encode
                 * @memberof exonum.consensus.TransactionsResponse
                 * @static
                 * @param {exonum.consensus.ITransactionsResponse} message TransactionsResponse message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                TransactionsResponse.encode = function encode(message, writer) {
                    if (!writer)
                        writer = $Writer.create();
                    if (message.to != null && message.hasOwnProperty("to"))
                        $root.exonum.PublicKey.encode(message.to, writer.uint32(/* id 1, wireType 2 =*/10).fork()).ldelim();
                    if (message.transactions != null && message.transactions.length)
                        for (var i = 0; i < message.transactions.length; ++i)
                            writer.uint32(/* id 2, wireType 2 =*/18).bytes(message.transactions[i]);
                    return writer;
                };
    
                /**
                 * Encodes the specified TransactionsResponse message, length delimited. Does not implicitly {@link exonum.consensus.TransactionsResponse.verify|verify} messages.
                 * @function encodeDelimited
                 * @memberof exonum.consensus.TransactionsResponse
                 * @static
                 * @param {exonum.consensus.ITransactionsResponse} message TransactionsResponse message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                TransactionsResponse.encodeDelimited = function encodeDelimited(message, writer) {
                    return this.encode(message, writer).ldelim();
                };
    
                /**
                 * Decodes a TransactionsResponse message from the specified reader or buffer.
                 * @function decode
                 * @memberof exonum.consensus.TransactionsResponse
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @param {number} [length] Message length if known beforehand
                 * @returns {exonum.consensus.TransactionsResponse} TransactionsResponse
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                TransactionsResponse.decode = function decode(reader, length) {
                    if (!(reader instanceof $Reader))
                        reader = $Reader.create(reader);
                    var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.consensus.TransactionsResponse();
                    while (reader.pos < end) {
                        var tag = reader.uint32();
                        switch (tag >>> 3) {
                        case 1:
                            message.to = $root.exonum.PublicKey.decode(reader, reader.uint32());
                            break;
                        case 2:
                            if (!(message.transactions && message.transactions.length))
                                message.transactions = [];
                            message.transactions.push(reader.bytes());
                            break;
                        default:
                            reader.skipType(tag & 7);
                            break;
                        }
                    }
                    return message;
                };
    
                /**
                 * Decodes a TransactionsResponse message from the specified reader or buffer, length delimited.
                 * @function decodeDelimited
                 * @memberof exonum.consensus.TransactionsResponse
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @returns {exonum.consensus.TransactionsResponse} TransactionsResponse
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                TransactionsResponse.decodeDelimited = function decodeDelimited(reader) {
                    if (!(reader instanceof $Reader))
                        reader = new $Reader(reader);
                    return this.decode(reader, reader.uint32());
                };
    
                /**
                 * Verifies a TransactionsResponse message.
                 * @function verify
                 * @memberof exonum.consensus.TransactionsResponse
                 * @static
                 * @param {Object.<string,*>} message Plain object to verify
                 * @returns {string|null} `null` if valid, otherwise the reason why it is not
                 */
                TransactionsResponse.verify = function verify(message) {
                    if (typeof message !== "object" || message === null)
                        return "object expected";
                    if (message.to != null && message.hasOwnProperty("to")) {
                        var error = $root.exonum.PublicKey.verify(message.to);
                        if (error)
                            return "to." + error;
                    }
                    if (message.transactions != null && message.hasOwnProperty("transactions")) {
                        if (!Array.isArray(message.transactions))
                            return "transactions: array expected";
                        for (var i = 0; i < message.transactions.length; ++i)
                            if (!(message.transactions[i] && typeof message.transactions[i].length === "number" || $util.isString(message.transactions[i])))
                                return "transactions: buffer[] expected";
                    }
                    return null;
                };
    
                /**
                 * Creates a TransactionsResponse message from a plain object. Also converts values to their respective internal types.
                 * @function fromObject
                 * @memberof exonum.consensus.TransactionsResponse
                 * @static
                 * @param {Object.<string,*>} object Plain object
                 * @returns {exonum.consensus.TransactionsResponse} TransactionsResponse
                 */
                TransactionsResponse.fromObject = function fromObject(object) {
                    if (object instanceof $root.exonum.consensus.TransactionsResponse)
                        return object;
                    var message = new $root.exonum.consensus.TransactionsResponse();
                    if (object.to != null) {
                        if (typeof object.to !== "object")
                            throw TypeError(".exonum.consensus.TransactionsResponse.to: object expected");
                        message.to = $root.exonum.PublicKey.fromObject(object.to);
                    }
                    if (object.transactions) {
                        if (!Array.isArray(object.transactions))
                            throw TypeError(".exonum.consensus.TransactionsResponse.transactions: array expected");
                        message.transactions = [];
                        for (var i = 0; i < object.transactions.length; ++i)
                            if (typeof object.transactions[i] === "string")
                                $util.base64.decode(object.transactions[i], message.transactions[i] = $util.newBuffer($util.base64.length(object.transactions[i])), 0);
                            else if (object.transactions[i].length)
                                message.transactions[i] = object.transactions[i];
                    }
                    return message;
                };
    
                /**
                 * Creates a plain object from a TransactionsResponse message. Also converts values to other types if specified.
                 * @function toObject
                 * @memberof exonum.consensus.TransactionsResponse
                 * @static
                 * @param {exonum.consensus.TransactionsResponse} message TransactionsResponse
                 * @param {$protobuf.IConversionOptions} [options] Conversion options
                 * @returns {Object.<string,*>} Plain object
                 */
                TransactionsResponse.toObject = function toObject(message, options) {
                    if (!options)
                        options = {};
                    var object = {};
                    if (options.arrays || options.defaults)
                        object.transactions = [];
                    if (options.defaults)
                        object.to = null;
                    if (message.to != null && message.hasOwnProperty("to"))
                        object.to = $root.exonum.PublicKey.toObject(message.to, options);
                    if (message.transactions && message.transactions.length) {
                        object.transactions = [];
                        for (var j = 0; j < message.transactions.length; ++j)
                            object.transactions[j] = options.bytes === String ? $util.base64.encode(message.transactions[j], 0, message.transactions[j].length) : options.bytes === Array ? Array.prototype.slice.call(message.transactions[j]) : message.transactions[j];
                    }
                    return object;
                };
    
                /**
                 * Converts this TransactionsResponse to JSON.
                 * @function toJSON
                 * @memberof exonum.consensus.TransactionsResponse
                 * @instance
                 * @returns {Object.<string,*>} JSON object
                 */
                TransactionsResponse.prototype.toJSON = function toJSON() {
                    return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                };
    
                return TransactionsResponse;
            })();
    
            consensus.ProposeRequest = (function() {
    
                /**
                 * Properties of a ProposeRequest.
                 * @memberof exonum.consensus
                 * @interface IProposeRequest
                 * @property {exonum.IPublicKey|null} [to] ProposeRequest to
                 * @property {number|Long|null} [height] ProposeRequest height
                 * @property {exonum.IHash|null} [proposeHash] ProposeRequest proposeHash
                 */
    
                /**
                 * Constructs a new ProposeRequest.
                 * @memberof exonum.consensus
                 * @classdesc Represents a ProposeRequest.
                 * @implements IProposeRequest
                 * @constructor
                 * @param {exonum.consensus.IProposeRequest=} [properties] Properties to set
                 */
                function ProposeRequest(properties) {
                    if (properties)
                        for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                            if (properties[keys[i]] != null)
                                this[keys[i]] = properties[keys[i]];
                }
    
                /**
                 * ProposeRequest to.
                 * @member {exonum.IPublicKey|null|undefined} to
                 * @memberof exonum.consensus.ProposeRequest
                 * @instance
                 */
                ProposeRequest.prototype.to = null;
    
                /**
                 * ProposeRequest height.
                 * @member {number|Long} height
                 * @memberof exonum.consensus.ProposeRequest
                 * @instance
                 */
                ProposeRequest.prototype.height = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
                /**
                 * ProposeRequest proposeHash.
                 * @member {exonum.IHash|null|undefined} proposeHash
                 * @memberof exonum.consensus.ProposeRequest
                 * @instance
                 */
                ProposeRequest.prototype.proposeHash = null;
    
                /**
                 * Creates a new ProposeRequest instance using the specified properties.
                 * @function create
                 * @memberof exonum.consensus.ProposeRequest
                 * @static
                 * @param {exonum.consensus.IProposeRequest=} [properties] Properties to set
                 * @returns {exonum.consensus.ProposeRequest} ProposeRequest instance
                 */
                ProposeRequest.create = function create(properties) {
                    return new ProposeRequest(properties);
                };
    
                /**
                 * Encodes the specified ProposeRequest message. Does not implicitly {@link exonum.consensus.ProposeRequest.verify|verify} messages.
                 * @function encode
                 * @memberof exonum.consensus.ProposeRequest
                 * @static
                 * @param {exonum.consensus.IProposeRequest} message ProposeRequest message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                ProposeRequest.encode = function encode(message, writer) {
                    if (!writer)
                        writer = $Writer.create();
                    if (message.to != null && message.hasOwnProperty("to"))
                        $root.exonum.PublicKey.encode(message.to, writer.uint32(/* id 1, wireType 2 =*/10).fork()).ldelim();
                    if (message.height != null && message.hasOwnProperty("height"))
                        writer.uint32(/* id 2, wireType 0 =*/16).uint64(message.height);
                    if (message.proposeHash != null && message.hasOwnProperty("proposeHash"))
                        $root.exonum.Hash.encode(message.proposeHash, writer.uint32(/* id 3, wireType 2 =*/26).fork()).ldelim();
                    return writer;
                };
    
                /**
                 * Encodes the specified ProposeRequest message, length delimited. Does not implicitly {@link exonum.consensus.ProposeRequest.verify|verify} messages.
                 * @function encodeDelimited
                 * @memberof exonum.consensus.ProposeRequest
                 * @static
                 * @param {exonum.consensus.IProposeRequest} message ProposeRequest message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                ProposeRequest.encodeDelimited = function encodeDelimited(message, writer) {
                    return this.encode(message, writer).ldelim();
                };
    
                /**
                 * Decodes a ProposeRequest message from the specified reader or buffer.
                 * @function decode
                 * @memberof exonum.consensus.ProposeRequest
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @param {number} [length] Message length if known beforehand
                 * @returns {exonum.consensus.ProposeRequest} ProposeRequest
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                ProposeRequest.decode = function decode(reader, length) {
                    if (!(reader instanceof $Reader))
                        reader = $Reader.create(reader);
                    var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.consensus.ProposeRequest();
                    while (reader.pos < end) {
                        var tag = reader.uint32();
                        switch (tag >>> 3) {
                        case 1:
                            message.to = $root.exonum.PublicKey.decode(reader, reader.uint32());
                            break;
                        case 2:
                            message.height = reader.uint64();
                            break;
                        case 3:
                            message.proposeHash = $root.exonum.Hash.decode(reader, reader.uint32());
                            break;
                        default:
                            reader.skipType(tag & 7);
                            break;
                        }
                    }
                    return message;
                };
    
                /**
                 * Decodes a ProposeRequest message from the specified reader or buffer, length delimited.
                 * @function decodeDelimited
                 * @memberof exonum.consensus.ProposeRequest
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @returns {exonum.consensus.ProposeRequest} ProposeRequest
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                ProposeRequest.decodeDelimited = function decodeDelimited(reader) {
                    if (!(reader instanceof $Reader))
                        reader = new $Reader(reader);
                    return this.decode(reader, reader.uint32());
                };
    
                /**
                 * Verifies a ProposeRequest message.
                 * @function verify
                 * @memberof exonum.consensus.ProposeRequest
                 * @static
                 * @param {Object.<string,*>} message Plain object to verify
                 * @returns {string|null} `null` if valid, otherwise the reason why it is not
                 */
                ProposeRequest.verify = function verify(message) {
                    if (typeof message !== "object" || message === null)
                        return "object expected";
                    if (message.to != null && message.hasOwnProperty("to")) {
                        var error = $root.exonum.PublicKey.verify(message.to);
                        if (error)
                            return "to." + error;
                    }
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (!$util.isInteger(message.height) && !(message.height && $util.isInteger(message.height.low) && $util.isInteger(message.height.high)))
                            return "height: integer|Long expected";
                    if (message.proposeHash != null && message.hasOwnProperty("proposeHash")) {
                        var error = $root.exonum.Hash.verify(message.proposeHash);
                        if (error)
                            return "proposeHash." + error;
                    }
                    return null;
                };
    
                /**
                 * Creates a ProposeRequest message from a plain object. Also converts values to their respective internal types.
                 * @function fromObject
                 * @memberof exonum.consensus.ProposeRequest
                 * @static
                 * @param {Object.<string,*>} object Plain object
                 * @returns {exonum.consensus.ProposeRequest} ProposeRequest
                 */
                ProposeRequest.fromObject = function fromObject(object) {
                    if (object instanceof $root.exonum.consensus.ProposeRequest)
                        return object;
                    var message = new $root.exonum.consensus.ProposeRequest();
                    if (object.to != null) {
                        if (typeof object.to !== "object")
                            throw TypeError(".exonum.consensus.ProposeRequest.to: object expected");
                        message.to = $root.exonum.PublicKey.fromObject(object.to);
                    }
                    if (object.height != null)
                        if ($util.Long)
                            (message.height = $util.Long.fromValue(object.height)).unsigned = true;
                        else if (typeof object.height === "string")
                            message.height = parseInt(object.height, 10);
                        else if (typeof object.height === "number")
                            message.height = object.height;
                        else if (typeof object.height === "object")
                            message.height = new $util.LongBits(object.height.low >>> 0, object.height.high >>> 0).toNumber(true);
                    if (object.proposeHash != null) {
                        if (typeof object.proposeHash !== "object")
                            throw TypeError(".exonum.consensus.ProposeRequest.proposeHash: object expected");
                        message.proposeHash = $root.exonum.Hash.fromObject(object.proposeHash);
                    }
                    return message;
                };
    
                /**
                 * Creates a plain object from a ProposeRequest message. Also converts values to other types if specified.
                 * @function toObject
                 * @memberof exonum.consensus.ProposeRequest
                 * @static
                 * @param {exonum.consensus.ProposeRequest} message ProposeRequest
                 * @param {$protobuf.IConversionOptions} [options] Conversion options
                 * @returns {Object.<string,*>} Plain object
                 */
                ProposeRequest.toObject = function toObject(message, options) {
                    if (!options)
                        options = {};
                    var object = {};
                    if (options.defaults) {
                        object.to = null;
                        if ($util.Long) {
                            var long = new $util.Long(0, 0, true);
                            object.height = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                        } else
                            object.height = options.longs === String ? "0" : 0;
                        object.proposeHash = null;
                    }
                    if (message.to != null && message.hasOwnProperty("to"))
                        object.to = $root.exonum.PublicKey.toObject(message.to, options);
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (typeof message.height === "number")
                            object.height = options.longs === String ? String(message.height) : message.height;
                        else
                            object.height = options.longs === String ? $util.Long.prototype.toString.call(message.height) : options.longs === Number ? new $util.LongBits(message.height.low >>> 0, message.height.high >>> 0).toNumber(true) : message.height;
                    if (message.proposeHash != null && message.hasOwnProperty("proposeHash"))
                        object.proposeHash = $root.exonum.Hash.toObject(message.proposeHash, options);
                    return object;
                };
    
                /**
                 * Converts this ProposeRequest to JSON.
                 * @function toJSON
                 * @memberof exonum.consensus.ProposeRequest
                 * @instance
                 * @returns {Object.<string,*>} JSON object
                 */
                ProposeRequest.prototype.toJSON = function toJSON() {
                    return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                };
    
                return ProposeRequest;
            })();
    
            consensus.TransactionsRequest = (function() {
    
                /**
                 * Properties of a TransactionsRequest.
                 * @memberof exonum.consensus
                 * @interface ITransactionsRequest
                 * @property {exonum.IPublicKey|null} [to] TransactionsRequest to
                 * @property {Array.<exonum.IHash>|null} [txs] TransactionsRequest txs
                 */
    
                /**
                 * Constructs a new TransactionsRequest.
                 * @memberof exonum.consensus
                 * @classdesc Represents a TransactionsRequest.
                 * @implements ITransactionsRequest
                 * @constructor
                 * @param {exonum.consensus.ITransactionsRequest=} [properties] Properties to set
                 */
                function TransactionsRequest(properties) {
                    this.txs = [];
                    if (properties)
                        for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                            if (properties[keys[i]] != null)
                                this[keys[i]] = properties[keys[i]];
                }
    
                /**
                 * TransactionsRequest to.
                 * @member {exonum.IPublicKey|null|undefined} to
                 * @memberof exonum.consensus.TransactionsRequest
                 * @instance
                 */
                TransactionsRequest.prototype.to = null;
    
                /**
                 * TransactionsRequest txs.
                 * @member {Array.<exonum.IHash>} txs
                 * @memberof exonum.consensus.TransactionsRequest
                 * @instance
                 */
                TransactionsRequest.prototype.txs = $util.emptyArray;
    
                /**
                 * Creates a new TransactionsRequest instance using the specified properties.
                 * @function create
                 * @memberof exonum.consensus.TransactionsRequest
                 * @static
                 * @param {exonum.consensus.ITransactionsRequest=} [properties] Properties to set
                 * @returns {exonum.consensus.TransactionsRequest} TransactionsRequest instance
                 */
                TransactionsRequest.create = function create(properties) {
                    return new TransactionsRequest(properties);
                };
    
                /**
                 * Encodes the specified TransactionsRequest message. Does not implicitly {@link exonum.consensus.TransactionsRequest.verify|verify} messages.
                 * @function encode
                 * @memberof exonum.consensus.TransactionsRequest
                 * @static
                 * @param {exonum.consensus.ITransactionsRequest} message TransactionsRequest message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                TransactionsRequest.encode = function encode(message, writer) {
                    if (!writer)
                        writer = $Writer.create();
                    if (message.to != null && message.hasOwnProperty("to"))
                        $root.exonum.PublicKey.encode(message.to, writer.uint32(/* id 1, wireType 2 =*/10).fork()).ldelim();
                    if (message.txs != null && message.txs.length)
                        for (var i = 0; i < message.txs.length; ++i)
                            $root.exonum.Hash.encode(message.txs[i], writer.uint32(/* id 2, wireType 2 =*/18).fork()).ldelim();
                    return writer;
                };
    
                /**
                 * Encodes the specified TransactionsRequest message, length delimited. Does not implicitly {@link exonum.consensus.TransactionsRequest.verify|verify} messages.
                 * @function encodeDelimited
                 * @memberof exonum.consensus.TransactionsRequest
                 * @static
                 * @param {exonum.consensus.ITransactionsRequest} message TransactionsRequest message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                TransactionsRequest.encodeDelimited = function encodeDelimited(message, writer) {
                    return this.encode(message, writer).ldelim();
                };
    
                /**
                 * Decodes a TransactionsRequest message from the specified reader or buffer.
                 * @function decode
                 * @memberof exonum.consensus.TransactionsRequest
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @param {number} [length] Message length if known beforehand
                 * @returns {exonum.consensus.TransactionsRequest} TransactionsRequest
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                TransactionsRequest.decode = function decode(reader, length) {
                    if (!(reader instanceof $Reader))
                        reader = $Reader.create(reader);
                    var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.consensus.TransactionsRequest();
                    while (reader.pos < end) {
                        var tag = reader.uint32();
                        switch (tag >>> 3) {
                        case 1:
                            message.to = $root.exonum.PublicKey.decode(reader, reader.uint32());
                            break;
                        case 2:
                            if (!(message.txs && message.txs.length))
                                message.txs = [];
                            message.txs.push($root.exonum.Hash.decode(reader, reader.uint32()));
                            break;
                        default:
                            reader.skipType(tag & 7);
                            break;
                        }
                    }
                    return message;
                };
    
                /**
                 * Decodes a TransactionsRequest message from the specified reader or buffer, length delimited.
                 * @function decodeDelimited
                 * @memberof exonum.consensus.TransactionsRequest
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @returns {exonum.consensus.TransactionsRequest} TransactionsRequest
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                TransactionsRequest.decodeDelimited = function decodeDelimited(reader) {
                    if (!(reader instanceof $Reader))
                        reader = new $Reader(reader);
                    return this.decode(reader, reader.uint32());
                };
    
                /**
                 * Verifies a TransactionsRequest message.
                 * @function verify
                 * @memberof exonum.consensus.TransactionsRequest
                 * @static
                 * @param {Object.<string,*>} message Plain object to verify
                 * @returns {string|null} `null` if valid, otherwise the reason why it is not
                 */
                TransactionsRequest.verify = function verify(message) {
                    if (typeof message !== "object" || message === null)
                        return "object expected";
                    if (message.to != null && message.hasOwnProperty("to")) {
                        var error = $root.exonum.PublicKey.verify(message.to);
                        if (error)
                            return "to." + error;
                    }
                    if (message.txs != null && message.hasOwnProperty("txs")) {
                        if (!Array.isArray(message.txs))
                            return "txs: array expected";
                        for (var i = 0; i < message.txs.length; ++i) {
                            var error = $root.exonum.Hash.verify(message.txs[i]);
                            if (error)
                                return "txs." + error;
                        }
                    }
                    return null;
                };
    
                /**
                 * Creates a TransactionsRequest message from a plain object. Also converts values to their respective internal types.
                 * @function fromObject
                 * @memberof exonum.consensus.TransactionsRequest
                 * @static
                 * @param {Object.<string,*>} object Plain object
                 * @returns {exonum.consensus.TransactionsRequest} TransactionsRequest
                 */
                TransactionsRequest.fromObject = function fromObject(object) {
                    if (object instanceof $root.exonum.consensus.TransactionsRequest)
                        return object;
                    var message = new $root.exonum.consensus.TransactionsRequest();
                    if (object.to != null) {
                        if (typeof object.to !== "object")
                            throw TypeError(".exonum.consensus.TransactionsRequest.to: object expected");
                        message.to = $root.exonum.PublicKey.fromObject(object.to);
                    }
                    if (object.txs) {
                        if (!Array.isArray(object.txs))
                            throw TypeError(".exonum.consensus.TransactionsRequest.txs: array expected");
                        message.txs = [];
                        for (var i = 0; i < object.txs.length; ++i) {
                            if (typeof object.txs[i] !== "object")
                                throw TypeError(".exonum.consensus.TransactionsRequest.txs: object expected");
                            message.txs[i] = $root.exonum.Hash.fromObject(object.txs[i]);
                        }
                    }
                    return message;
                };
    
                /**
                 * Creates a plain object from a TransactionsRequest message. Also converts values to other types if specified.
                 * @function toObject
                 * @memberof exonum.consensus.TransactionsRequest
                 * @static
                 * @param {exonum.consensus.TransactionsRequest} message TransactionsRequest
                 * @param {$protobuf.IConversionOptions} [options] Conversion options
                 * @returns {Object.<string,*>} Plain object
                 */
                TransactionsRequest.toObject = function toObject(message, options) {
                    if (!options)
                        options = {};
                    var object = {};
                    if (options.arrays || options.defaults)
                        object.txs = [];
                    if (options.defaults)
                        object.to = null;
                    if (message.to != null && message.hasOwnProperty("to"))
                        object.to = $root.exonum.PublicKey.toObject(message.to, options);
                    if (message.txs && message.txs.length) {
                        object.txs = [];
                        for (var j = 0; j < message.txs.length; ++j)
                            object.txs[j] = $root.exonum.Hash.toObject(message.txs[j], options);
                    }
                    return object;
                };
    
                /**
                 * Converts this TransactionsRequest to JSON.
                 * @function toJSON
                 * @memberof exonum.consensus.TransactionsRequest
                 * @instance
                 * @returns {Object.<string,*>} JSON object
                 */
                TransactionsRequest.prototype.toJSON = function toJSON() {
                    return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                };
    
                return TransactionsRequest;
            })();
    
            consensus.PrevotesRequest = (function() {
    
                /**
                 * Properties of a PrevotesRequest.
                 * @memberof exonum.consensus
                 * @interface IPrevotesRequest
                 * @property {exonum.IPublicKey|null} [to] PrevotesRequest to
                 * @property {number|Long|null} [height] PrevotesRequest height
                 * @property {number|null} [round] PrevotesRequest round
                 * @property {exonum.IHash|null} [proposeHash] PrevotesRequest proposeHash
                 * @property {exonum.IBitVec|null} [validators] PrevotesRequest validators
                 */
    
                /**
                 * Constructs a new PrevotesRequest.
                 * @memberof exonum.consensus
                 * @classdesc Represents a PrevotesRequest.
                 * @implements IPrevotesRequest
                 * @constructor
                 * @param {exonum.consensus.IPrevotesRequest=} [properties] Properties to set
                 */
                function PrevotesRequest(properties) {
                    if (properties)
                        for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                            if (properties[keys[i]] != null)
                                this[keys[i]] = properties[keys[i]];
                }
    
                /**
                 * PrevotesRequest to.
                 * @member {exonum.IPublicKey|null|undefined} to
                 * @memberof exonum.consensus.PrevotesRequest
                 * @instance
                 */
                PrevotesRequest.prototype.to = null;
    
                /**
                 * PrevotesRequest height.
                 * @member {number|Long} height
                 * @memberof exonum.consensus.PrevotesRequest
                 * @instance
                 */
                PrevotesRequest.prototype.height = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
                /**
                 * PrevotesRequest round.
                 * @member {number} round
                 * @memberof exonum.consensus.PrevotesRequest
                 * @instance
                 */
                PrevotesRequest.prototype.round = 0;
    
                /**
                 * PrevotesRequest proposeHash.
                 * @member {exonum.IHash|null|undefined} proposeHash
                 * @memberof exonum.consensus.PrevotesRequest
                 * @instance
                 */
                PrevotesRequest.prototype.proposeHash = null;
    
                /**
                 * PrevotesRequest validators.
                 * @member {exonum.IBitVec|null|undefined} validators
                 * @memberof exonum.consensus.PrevotesRequest
                 * @instance
                 */
                PrevotesRequest.prototype.validators = null;
    
                /**
                 * Creates a new PrevotesRequest instance using the specified properties.
                 * @function create
                 * @memberof exonum.consensus.PrevotesRequest
                 * @static
                 * @param {exonum.consensus.IPrevotesRequest=} [properties] Properties to set
                 * @returns {exonum.consensus.PrevotesRequest} PrevotesRequest instance
                 */
                PrevotesRequest.create = function create(properties) {
                    return new PrevotesRequest(properties);
                };
    
                /**
                 * Encodes the specified PrevotesRequest message. Does not implicitly {@link exonum.consensus.PrevotesRequest.verify|verify} messages.
                 * @function encode
                 * @memberof exonum.consensus.PrevotesRequest
                 * @static
                 * @param {exonum.consensus.IPrevotesRequest} message PrevotesRequest message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                PrevotesRequest.encode = function encode(message, writer) {
                    if (!writer)
                        writer = $Writer.create();
                    if (message.to != null && message.hasOwnProperty("to"))
                        $root.exonum.PublicKey.encode(message.to, writer.uint32(/* id 1, wireType 2 =*/10).fork()).ldelim();
                    if (message.height != null && message.hasOwnProperty("height"))
                        writer.uint32(/* id 2, wireType 0 =*/16).uint64(message.height);
                    if (message.round != null && message.hasOwnProperty("round"))
                        writer.uint32(/* id 3, wireType 0 =*/24).uint32(message.round);
                    if (message.proposeHash != null && message.hasOwnProperty("proposeHash"))
                        $root.exonum.Hash.encode(message.proposeHash, writer.uint32(/* id 4, wireType 2 =*/34).fork()).ldelim();
                    if (message.validators != null && message.hasOwnProperty("validators"))
                        $root.exonum.BitVec.encode(message.validators, writer.uint32(/* id 5, wireType 2 =*/42).fork()).ldelim();
                    return writer;
                };
    
                /**
                 * Encodes the specified PrevotesRequest message, length delimited. Does not implicitly {@link exonum.consensus.PrevotesRequest.verify|verify} messages.
                 * @function encodeDelimited
                 * @memberof exonum.consensus.PrevotesRequest
                 * @static
                 * @param {exonum.consensus.IPrevotesRequest} message PrevotesRequest message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                PrevotesRequest.encodeDelimited = function encodeDelimited(message, writer) {
                    return this.encode(message, writer).ldelim();
                };
    
                /**
                 * Decodes a PrevotesRequest message from the specified reader or buffer.
                 * @function decode
                 * @memberof exonum.consensus.PrevotesRequest
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @param {number} [length] Message length if known beforehand
                 * @returns {exonum.consensus.PrevotesRequest} PrevotesRequest
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                PrevotesRequest.decode = function decode(reader, length) {
                    if (!(reader instanceof $Reader))
                        reader = $Reader.create(reader);
                    var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.consensus.PrevotesRequest();
                    while (reader.pos < end) {
                        var tag = reader.uint32();
                        switch (tag >>> 3) {
                        case 1:
                            message.to = $root.exonum.PublicKey.decode(reader, reader.uint32());
                            break;
                        case 2:
                            message.height = reader.uint64();
                            break;
                        case 3:
                            message.round = reader.uint32();
                            break;
                        case 4:
                            message.proposeHash = $root.exonum.Hash.decode(reader, reader.uint32());
                            break;
                        case 5:
                            message.validators = $root.exonum.BitVec.decode(reader, reader.uint32());
                            break;
                        default:
                            reader.skipType(tag & 7);
                            break;
                        }
                    }
                    return message;
                };
    
                /**
                 * Decodes a PrevotesRequest message from the specified reader or buffer, length delimited.
                 * @function decodeDelimited
                 * @memberof exonum.consensus.PrevotesRequest
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @returns {exonum.consensus.PrevotesRequest} PrevotesRequest
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                PrevotesRequest.decodeDelimited = function decodeDelimited(reader) {
                    if (!(reader instanceof $Reader))
                        reader = new $Reader(reader);
                    return this.decode(reader, reader.uint32());
                };
    
                /**
                 * Verifies a PrevotesRequest message.
                 * @function verify
                 * @memberof exonum.consensus.PrevotesRequest
                 * @static
                 * @param {Object.<string,*>} message Plain object to verify
                 * @returns {string|null} `null` if valid, otherwise the reason why it is not
                 */
                PrevotesRequest.verify = function verify(message) {
                    if (typeof message !== "object" || message === null)
                        return "object expected";
                    if (message.to != null && message.hasOwnProperty("to")) {
                        var error = $root.exonum.PublicKey.verify(message.to);
                        if (error)
                            return "to." + error;
                    }
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (!$util.isInteger(message.height) && !(message.height && $util.isInteger(message.height.low) && $util.isInteger(message.height.high)))
                            return "height: integer|Long expected";
                    if (message.round != null && message.hasOwnProperty("round"))
                        if (!$util.isInteger(message.round))
                            return "round: integer expected";
                    if (message.proposeHash != null && message.hasOwnProperty("proposeHash")) {
                        var error = $root.exonum.Hash.verify(message.proposeHash);
                        if (error)
                            return "proposeHash." + error;
                    }
                    if (message.validators != null && message.hasOwnProperty("validators")) {
                        var error = $root.exonum.BitVec.verify(message.validators);
                        if (error)
                            return "validators." + error;
                    }
                    return null;
                };
    
                /**
                 * Creates a PrevotesRequest message from a plain object. Also converts values to their respective internal types.
                 * @function fromObject
                 * @memberof exonum.consensus.PrevotesRequest
                 * @static
                 * @param {Object.<string,*>} object Plain object
                 * @returns {exonum.consensus.PrevotesRequest} PrevotesRequest
                 */
                PrevotesRequest.fromObject = function fromObject(object) {
                    if (object instanceof $root.exonum.consensus.PrevotesRequest)
                        return object;
                    var message = new $root.exonum.consensus.PrevotesRequest();
                    if (object.to != null) {
                        if (typeof object.to !== "object")
                            throw TypeError(".exonum.consensus.PrevotesRequest.to: object expected");
                        message.to = $root.exonum.PublicKey.fromObject(object.to);
                    }
                    if (object.height != null)
                        if ($util.Long)
                            (message.height = $util.Long.fromValue(object.height)).unsigned = true;
                        else if (typeof object.height === "string")
                            message.height = parseInt(object.height, 10);
                        else if (typeof object.height === "number")
                            message.height = object.height;
                        else if (typeof object.height === "object")
                            message.height = new $util.LongBits(object.height.low >>> 0, object.height.high >>> 0).toNumber(true);
                    if (object.round != null)
                        message.round = object.round >>> 0;
                    if (object.proposeHash != null) {
                        if (typeof object.proposeHash !== "object")
                            throw TypeError(".exonum.consensus.PrevotesRequest.proposeHash: object expected");
                        message.proposeHash = $root.exonum.Hash.fromObject(object.proposeHash);
                    }
                    if (object.validators != null) {
                        if (typeof object.validators !== "object")
                            throw TypeError(".exonum.consensus.PrevotesRequest.validators: object expected");
                        message.validators = $root.exonum.BitVec.fromObject(object.validators);
                    }
                    return message;
                };
    
                /**
                 * Creates a plain object from a PrevotesRequest message. Also converts values to other types if specified.
                 * @function toObject
                 * @memberof exonum.consensus.PrevotesRequest
                 * @static
                 * @param {exonum.consensus.PrevotesRequest} message PrevotesRequest
                 * @param {$protobuf.IConversionOptions} [options] Conversion options
                 * @returns {Object.<string,*>} Plain object
                 */
                PrevotesRequest.toObject = function toObject(message, options) {
                    if (!options)
                        options = {};
                    var object = {};
                    if (options.defaults) {
                        object.to = null;
                        if ($util.Long) {
                            var long = new $util.Long(0, 0, true);
                            object.height = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                        } else
                            object.height = options.longs === String ? "0" : 0;
                        object.round = 0;
                        object.proposeHash = null;
                        object.validators = null;
                    }
                    if (message.to != null && message.hasOwnProperty("to"))
                        object.to = $root.exonum.PublicKey.toObject(message.to, options);
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (typeof message.height === "number")
                            object.height = options.longs === String ? String(message.height) : message.height;
                        else
                            object.height = options.longs === String ? $util.Long.prototype.toString.call(message.height) : options.longs === Number ? new $util.LongBits(message.height.low >>> 0, message.height.high >>> 0).toNumber(true) : message.height;
                    if (message.round != null && message.hasOwnProperty("round"))
                        object.round = message.round;
                    if (message.proposeHash != null && message.hasOwnProperty("proposeHash"))
                        object.proposeHash = $root.exonum.Hash.toObject(message.proposeHash, options);
                    if (message.validators != null && message.hasOwnProperty("validators"))
                        object.validators = $root.exonum.BitVec.toObject(message.validators, options);
                    return object;
                };
    
                /**
                 * Converts this PrevotesRequest to JSON.
                 * @function toJSON
                 * @memberof exonum.consensus.PrevotesRequest
                 * @instance
                 * @returns {Object.<string,*>} JSON object
                 */
                PrevotesRequest.prototype.toJSON = function toJSON() {
                    return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                };
    
                return PrevotesRequest;
            })();
    
            consensus.PeersRequest = (function() {
    
                /**
                 * Properties of a PeersRequest.
                 * @memberof exonum.consensus
                 * @interface IPeersRequest
                 * @property {exonum.IPublicKey|null} [to] PeersRequest to
                 */
    
                /**
                 * Constructs a new PeersRequest.
                 * @memberof exonum.consensus
                 * @classdesc Represents a PeersRequest.
                 * @implements IPeersRequest
                 * @constructor
                 * @param {exonum.consensus.IPeersRequest=} [properties] Properties to set
                 */
                function PeersRequest(properties) {
                    if (properties)
                        for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                            if (properties[keys[i]] != null)
                                this[keys[i]] = properties[keys[i]];
                }
    
                /**
                 * PeersRequest to.
                 * @member {exonum.IPublicKey|null|undefined} to
                 * @memberof exonum.consensus.PeersRequest
                 * @instance
                 */
                PeersRequest.prototype.to = null;
    
                /**
                 * Creates a new PeersRequest instance using the specified properties.
                 * @function create
                 * @memberof exonum.consensus.PeersRequest
                 * @static
                 * @param {exonum.consensus.IPeersRequest=} [properties] Properties to set
                 * @returns {exonum.consensus.PeersRequest} PeersRequest instance
                 */
                PeersRequest.create = function create(properties) {
                    return new PeersRequest(properties);
                };
    
                /**
                 * Encodes the specified PeersRequest message. Does not implicitly {@link exonum.consensus.PeersRequest.verify|verify} messages.
                 * @function encode
                 * @memberof exonum.consensus.PeersRequest
                 * @static
                 * @param {exonum.consensus.IPeersRequest} message PeersRequest message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                PeersRequest.encode = function encode(message, writer) {
                    if (!writer)
                        writer = $Writer.create();
                    if (message.to != null && message.hasOwnProperty("to"))
                        $root.exonum.PublicKey.encode(message.to, writer.uint32(/* id 1, wireType 2 =*/10).fork()).ldelim();
                    return writer;
                };
    
                /**
                 * Encodes the specified PeersRequest message, length delimited. Does not implicitly {@link exonum.consensus.PeersRequest.verify|verify} messages.
                 * @function encodeDelimited
                 * @memberof exonum.consensus.PeersRequest
                 * @static
                 * @param {exonum.consensus.IPeersRequest} message PeersRequest message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                PeersRequest.encodeDelimited = function encodeDelimited(message, writer) {
                    return this.encode(message, writer).ldelim();
                };
    
                /**
                 * Decodes a PeersRequest message from the specified reader or buffer.
                 * @function decode
                 * @memberof exonum.consensus.PeersRequest
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @param {number} [length] Message length if known beforehand
                 * @returns {exonum.consensus.PeersRequest} PeersRequest
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                PeersRequest.decode = function decode(reader, length) {
                    if (!(reader instanceof $Reader))
                        reader = $Reader.create(reader);
                    var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.consensus.PeersRequest();
                    while (reader.pos < end) {
                        var tag = reader.uint32();
                        switch (tag >>> 3) {
                        case 1:
                            message.to = $root.exonum.PublicKey.decode(reader, reader.uint32());
                            break;
                        default:
                            reader.skipType(tag & 7);
                            break;
                        }
                    }
                    return message;
                };
    
                /**
                 * Decodes a PeersRequest message from the specified reader or buffer, length delimited.
                 * @function decodeDelimited
                 * @memberof exonum.consensus.PeersRequest
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @returns {exonum.consensus.PeersRequest} PeersRequest
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                PeersRequest.decodeDelimited = function decodeDelimited(reader) {
                    if (!(reader instanceof $Reader))
                        reader = new $Reader(reader);
                    return this.decode(reader, reader.uint32());
                };
    
                /**
                 * Verifies a PeersRequest message.
                 * @function verify
                 * @memberof exonum.consensus.PeersRequest
                 * @static
                 * @param {Object.<string,*>} message Plain object to verify
                 * @returns {string|null} `null` if valid, otherwise the reason why it is not
                 */
                PeersRequest.verify = function verify(message) {
                    if (typeof message !== "object" || message === null)
                        return "object expected";
                    if (message.to != null && message.hasOwnProperty("to")) {
                        var error = $root.exonum.PublicKey.verify(message.to);
                        if (error)
                            return "to." + error;
                    }
                    return null;
                };
    
                /**
                 * Creates a PeersRequest message from a plain object. Also converts values to their respective internal types.
                 * @function fromObject
                 * @memberof exonum.consensus.PeersRequest
                 * @static
                 * @param {Object.<string,*>} object Plain object
                 * @returns {exonum.consensus.PeersRequest} PeersRequest
                 */
                PeersRequest.fromObject = function fromObject(object) {
                    if (object instanceof $root.exonum.consensus.PeersRequest)
                        return object;
                    var message = new $root.exonum.consensus.PeersRequest();
                    if (object.to != null) {
                        if (typeof object.to !== "object")
                            throw TypeError(".exonum.consensus.PeersRequest.to: object expected");
                        message.to = $root.exonum.PublicKey.fromObject(object.to);
                    }
                    return message;
                };
    
                /**
                 * Creates a plain object from a PeersRequest message. Also converts values to other types if specified.
                 * @function toObject
                 * @memberof exonum.consensus.PeersRequest
                 * @static
                 * @param {exonum.consensus.PeersRequest} message PeersRequest
                 * @param {$protobuf.IConversionOptions} [options] Conversion options
                 * @returns {Object.<string,*>} Plain object
                 */
                PeersRequest.toObject = function toObject(message, options) {
                    if (!options)
                        options = {};
                    var object = {};
                    if (options.defaults)
                        object.to = null;
                    if (message.to != null && message.hasOwnProperty("to"))
                        object.to = $root.exonum.PublicKey.toObject(message.to, options);
                    return object;
                };
    
                /**
                 * Converts this PeersRequest to JSON.
                 * @function toJSON
                 * @memberof exonum.consensus.PeersRequest
                 * @instance
                 * @returns {Object.<string,*>} JSON object
                 */
                PeersRequest.prototype.toJSON = function toJSON() {
                    return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                };
    
                return PeersRequest;
            })();
    
            consensus.BlockRequest = (function() {
    
                /**
                 * Properties of a BlockRequest.
                 * @memberof exonum.consensus
                 * @interface IBlockRequest
                 * @property {exonum.IPublicKey|null} [to] BlockRequest to
                 * @property {number|Long|null} [height] BlockRequest height
                 */
    
                /**
                 * Constructs a new BlockRequest.
                 * @memberof exonum.consensus
                 * @classdesc Represents a BlockRequest.
                 * @implements IBlockRequest
                 * @constructor
                 * @param {exonum.consensus.IBlockRequest=} [properties] Properties to set
                 */
                function BlockRequest(properties) {
                    if (properties)
                        for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                            if (properties[keys[i]] != null)
                                this[keys[i]] = properties[keys[i]];
                }
    
                /**
                 * BlockRequest to.
                 * @member {exonum.IPublicKey|null|undefined} to
                 * @memberof exonum.consensus.BlockRequest
                 * @instance
                 */
                BlockRequest.prototype.to = null;
    
                /**
                 * BlockRequest height.
                 * @member {number|Long} height
                 * @memberof exonum.consensus.BlockRequest
                 * @instance
                 */
                BlockRequest.prototype.height = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
                /**
                 * Creates a new BlockRequest instance using the specified properties.
                 * @function create
                 * @memberof exonum.consensus.BlockRequest
                 * @static
                 * @param {exonum.consensus.IBlockRequest=} [properties] Properties to set
                 * @returns {exonum.consensus.BlockRequest} BlockRequest instance
                 */
                BlockRequest.create = function create(properties) {
                    return new BlockRequest(properties);
                };
    
                /**
                 * Encodes the specified BlockRequest message. Does not implicitly {@link exonum.consensus.BlockRequest.verify|verify} messages.
                 * @function encode
                 * @memberof exonum.consensus.BlockRequest
                 * @static
                 * @param {exonum.consensus.IBlockRequest} message BlockRequest message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                BlockRequest.encode = function encode(message, writer) {
                    if (!writer)
                        writer = $Writer.create();
                    if (message.to != null && message.hasOwnProperty("to"))
                        $root.exonum.PublicKey.encode(message.to, writer.uint32(/* id 1, wireType 2 =*/10).fork()).ldelim();
                    if (message.height != null && message.hasOwnProperty("height"))
                        writer.uint32(/* id 2, wireType 0 =*/16).uint64(message.height);
                    return writer;
                };
    
                /**
                 * Encodes the specified BlockRequest message, length delimited. Does not implicitly {@link exonum.consensus.BlockRequest.verify|verify} messages.
                 * @function encodeDelimited
                 * @memberof exonum.consensus.BlockRequest
                 * @static
                 * @param {exonum.consensus.IBlockRequest} message BlockRequest message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                BlockRequest.encodeDelimited = function encodeDelimited(message, writer) {
                    return this.encode(message, writer).ldelim();
                };
    
                /**
                 * Decodes a BlockRequest message from the specified reader or buffer.
                 * @function decode
                 * @memberof exonum.consensus.BlockRequest
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @param {number} [length] Message length if known beforehand
                 * @returns {exonum.consensus.BlockRequest} BlockRequest
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                BlockRequest.decode = function decode(reader, length) {
                    if (!(reader instanceof $Reader))
                        reader = $Reader.create(reader);
                    var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.consensus.BlockRequest();
                    while (reader.pos < end) {
                        var tag = reader.uint32();
                        switch (tag >>> 3) {
                        case 1:
                            message.to = $root.exonum.PublicKey.decode(reader, reader.uint32());
                            break;
                        case 2:
                            message.height = reader.uint64();
                            break;
                        default:
                            reader.skipType(tag & 7);
                            break;
                        }
                    }
                    return message;
                };
    
                /**
                 * Decodes a BlockRequest message from the specified reader or buffer, length delimited.
                 * @function decodeDelimited
                 * @memberof exonum.consensus.BlockRequest
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @returns {exonum.consensus.BlockRequest} BlockRequest
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                BlockRequest.decodeDelimited = function decodeDelimited(reader) {
                    if (!(reader instanceof $Reader))
                        reader = new $Reader(reader);
                    return this.decode(reader, reader.uint32());
                };
    
                /**
                 * Verifies a BlockRequest message.
                 * @function verify
                 * @memberof exonum.consensus.BlockRequest
                 * @static
                 * @param {Object.<string,*>} message Plain object to verify
                 * @returns {string|null} `null` if valid, otherwise the reason why it is not
                 */
                BlockRequest.verify = function verify(message) {
                    if (typeof message !== "object" || message === null)
                        return "object expected";
                    if (message.to != null && message.hasOwnProperty("to")) {
                        var error = $root.exonum.PublicKey.verify(message.to);
                        if (error)
                            return "to." + error;
                    }
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (!$util.isInteger(message.height) && !(message.height && $util.isInteger(message.height.low) && $util.isInteger(message.height.high)))
                            return "height: integer|Long expected";
                    return null;
                };
    
                /**
                 * Creates a BlockRequest message from a plain object. Also converts values to their respective internal types.
                 * @function fromObject
                 * @memberof exonum.consensus.BlockRequest
                 * @static
                 * @param {Object.<string,*>} object Plain object
                 * @returns {exonum.consensus.BlockRequest} BlockRequest
                 */
                BlockRequest.fromObject = function fromObject(object) {
                    if (object instanceof $root.exonum.consensus.BlockRequest)
                        return object;
                    var message = new $root.exonum.consensus.BlockRequest();
                    if (object.to != null) {
                        if (typeof object.to !== "object")
                            throw TypeError(".exonum.consensus.BlockRequest.to: object expected");
                        message.to = $root.exonum.PublicKey.fromObject(object.to);
                    }
                    if (object.height != null)
                        if ($util.Long)
                            (message.height = $util.Long.fromValue(object.height)).unsigned = true;
                        else if (typeof object.height === "string")
                            message.height = parseInt(object.height, 10);
                        else if (typeof object.height === "number")
                            message.height = object.height;
                        else if (typeof object.height === "object")
                            message.height = new $util.LongBits(object.height.low >>> 0, object.height.high >>> 0).toNumber(true);
                    return message;
                };
    
                /**
                 * Creates a plain object from a BlockRequest message. Also converts values to other types if specified.
                 * @function toObject
                 * @memberof exonum.consensus.BlockRequest
                 * @static
                 * @param {exonum.consensus.BlockRequest} message BlockRequest
                 * @param {$protobuf.IConversionOptions} [options] Conversion options
                 * @returns {Object.<string,*>} Plain object
                 */
                BlockRequest.toObject = function toObject(message, options) {
                    if (!options)
                        options = {};
                    var object = {};
                    if (options.defaults) {
                        object.to = null;
                        if ($util.Long) {
                            var long = new $util.Long(0, 0, true);
                            object.height = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                        } else
                            object.height = options.longs === String ? "0" : 0;
                    }
                    if (message.to != null && message.hasOwnProperty("to"))
                        object.to = $root.exonum.PublicKey.toObject(message.to, options);
                    if (message.height != null && message.hasOwnProperty("height"))
                        if (typeof message.height === "number")
                            object.height = options.longs === String ? String(message.height) : message.height;
                        else
                            object.height = options.longs === String ? $util.Long.prototype.toString.call(message.height) : options.longs === Number ? new $util.LongBits(message.height.low >>> 0, message.height.high >>> 0).toNumber(true) : message.height;
                    return object;
                };
    
                /**
                 * Converts this BlockRequest to JSON.
                 * @function toJSON
                 * @memberof exonum.consensus.BlockRequest
                 * @instance
                 * @returns {Object.<string,*>} JSON object
                 */
                BlockRequest.prototype.toJSON = function toJSON() {
                    return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                };
    
                return BlockRequest;
            })();
    
            return consensus;
        })();
    
        exonum.Hash = (function() {
    
            /**
             * Properties of a Hash.
             * @memberof exonum
             * @interface IHash
             * @property {Uint8Array|null} [data] Hash data
             */
    
            /**
             * Constructs a new Hash.
             * @memberof exonum
             * @classdesc Represents a Hash.
             * @implements IHash
             * @constructor
             * @param {exonum.IHash=} [properties] Properties to set
             */
            function Hash(properties) {
                if (properties)
                    for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                        if (properties[keys[i]] != null)
                            this[keys[i]] = properties[keys[i]];
            }
    
            /**
             * Hash data.
             * @member {Uint8Array} data
             * @memberof exonum.Hash
             * @instance
             */
            Hash.prototype.data = $util.newBuffer([]);
    
            /**
             * Creates a new Hash instance using the specified properties.
             * @function create
             * @memberof exonum.Hash
             * @static
             * @param {exonum.IHash=} [properties] Properties to set
             * @returns {exonum.Hash} Hash instance
             */
            Hash.create = function create(properties) {
                return new Hash(properties);
            };
    
            /**
             * Encodes the specified Hash message. Does not implicitly {@link exonum.Hash.verify|verify} messages.
             * @function encode
             * @memberof exonum.Hash
             * @static
             * @param {exonum.IHash} message Hash message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            Hash.encode = function encode(message, writer) {
                if (!writer)
                    writer = $Writer.create();
                if (message.data != null && message.hasOwnProperty("data"))
                    writer.uint32(/* id 1, wireType 2 =*/10).bytes(message.data);
                return writer;
            };
    
            /**
             * Encodes the specified Hash message, length delimited. Does not implicitly {@link exonum.Hash.verify|verify} messages.
             * @function encodeDelimited
             * @memberof exonum.Hash
             * @static
             * @param {exonum.IHash} message Hash message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            Hash.encodeDelimited = function encodeDelimited(message, writer) {
                return this.encode(message, writer).ldelim();
            };
    
            /**
             * Decodes a Hash message from the specified reader or buffer.
             * @function decode
             * @memberof exonum.Hash
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @param {number} [length] Message length if known beforehand
             * @returns {exonum.Hash} Hash
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            Hash.decode = function decode(reader, length) {
                if (!(reader instanceof $Reader))
                    reader = $Reader.create(reader);
                var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.Hash();
                while (reader.pos < end) {
                    var tag = reader.uint32();
                    switch (tag >>> 3) {
                    case 1:
                        message.data = reader.bytes();
                        break;
                    default:
                        reader.skipType(tag & 7);
                        break;
                    }
                }
                return message;
            };
    
            /**
             * Decodes a Hash message from the specified reader or buffer, length delimited.
             * @function decodeDelimited
             * @memberof exonum.Hash
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @returns {exonum.Hash} Hash
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            Hash.decodeDelimited = function decodeDelimited(reader) {
                if (!(reader instanceof $Reader))
                    reader = new $Reader(reader);
                return this.decode(reader, reader.uint32());
            };
    
            /**
             * Verifies a Hash message.
             * @function verify
             * @memberof exonum.Hash
             * @static
             * @param {Object.<string,*>} message Plain object to verify
             * @returns {string|null} `null` if valid, otherwise the reason why it is not
             */
            Hash.verify = function verify(message) {
                if (typeof message !== "object" || message === null)
                    return "object expected";
                if (message.data != null && message.hasOwnProperty("data"))
                    if (!(message.data && typeof message.data.length === "number" || $util.isString(message.data)))
                        return "data: buffer expected";
                return null;
            };
    
            /**
             * Creates a Hash message from a plain object. Also converts values to their respective internal types.
             * @function fromObject
             * @memberof exonum.Hash
             * @static
             * @param {Object.<string,*>} object Plain object
             * @returns {exonum.Hash} Hash
             */
            Hash.fromObject = function fromObject(object) {
                if (object instanceof $root.exonum.Hash)
                    return object;
                var message = new $root.exonum.Hash();
                if (object.data != null)
                    if (typeof object.data === "string")
                        $util.base64.decode(object.data, message.data = $util.newBuffer($util.base64.length(object.data)), 0);
                    else if (object.data.length)
                        message.data = object.data;
                return message;
            };
    
            /**
             * Creates a plain object from a Hash message. Also converts values to other types if specified.
             * @function toObject
             * @memberof exonum.Hash
             * @static
             * @param {exonum.Hash} message Hash
             * @param {$protobuf.IConversionOptions} [options] Conversion options
             * @returns {Object.<string,*>} Plain object
             */
            Hash.toObject = function toObject(message, options) {
                if (!options)
                    options = {};
                var object = {};
                if (options.defaults)
                    if (options.bytes === String)
                        object.data = "";
                    else {
                        object.data = [];
                        if (options.bytes !== Array)
                            object.data = $util.newBuffer(object.data);
                    }
                if (message.data != null && message.hasOwnProperty("data"))
                    object.data = options.bytes === String ? $util.base64.encode(message.data, 0, message.data.length) : options.bytes === Array ? Array.prototype.slice.call(message.data) : message.data;
                return object;
            };
    
            /**
             * Converts this Hash to JSON.
             * @function toJSON
             * @memberof exonum.Hash
             * @instance
             * @returns {Object.<string,*>} JSON object
             */
            Hash.prototype.toJSON = function toJSON() {
                return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
            };
    
            return Hash;
        })();
    
        exonum.PublicKey = (function() {
    
            /**
             * Properties of a PublicKey.
             * @memberof exonum
             * @interface IPublicKey
             * @property {Uint8Array|null} [data] PublicKey data
             */
    
            /**
             * Constructs a new PublicKey.
             * @memberof exonum
             * @classdesc Represents a PublicKey.
             * @implements IPublicKey
             * @constructor
             * @param {exonum.IPublicKey=} [properties] Properties to set
             */
            function PublicKey(properties) {
                if (properties)
                    for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                        if (properties[keys[i]] != null)
                            this[keys[i]] = properties[keys[i]];
            }
    
            /**
             * PublicKey data.
             * @member {Uint8Array} data
             * @memberof exonum.PublicKey
             * @instance
             */
            PublicKey.prototype.data = $util.newBuffer([]);
    
            /**
             * Creates a new PublicKey instance using the specified properties.
             * @function create
             * @memberof exonum.PublicKey
             * @static
             * @param {exonum.IPublicKey=} [properties] Properties to set
             * @returns {exonum.PublicKey} PublicKey instance
             */
            PublicKey.create = function create(properties) {
                return new PublicKey(properties);
            };
    
            /**
             * Encodes the specified PublicKey message. Does not implicitly {@link exonum.PublicKey.verify|verify} messages.
             * @function encode
             * @memberof exonum.PublicKey
             * @static
             * @param {exonum.IPublicKey} message PublicKey message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            PublicKey.encode = function encode(message, writer) {
                if (!writer)
                    writer = $Writer.create();
                if (message.data != null && message.hasOwnProperty("data"))
                    writer.uint32(/* id 1, wireType 2 =*/10).bytes(message.data);
                return writer;
            };
    
            /**
             * Encodes the specified PublicKey message, length delimited. Does not implicitly {@link exonum.PublicKey.verify|verify} messages.
             * @function encodeDelimited
             * @memberof exonum.PublicKey
             * @static
             * @param {exonum.IPublicKey} message PublicKey message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            PublicKey.encodeDelimited = function encodeDelimited(message, writer) {
                return this.encode(message, writer).ldelim();
            };
    
            /**
             * Decodes a PublicKey message from the specified reader or buffer.
             * @function decode
             * @memberof exonum.PublicKey
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @param {number} [length] Message length if known beforehand
             * @returns {exonum.PublicKey} PublicKey
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            PublicKey.decode = function decode(reader, length) {
                if (!(reader instanceof $Reader))
                    reader = $Reader.create(reader);
                var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.PublicKey();
                while (reader.pos < end) {
                    var tag = reader.uint32();
                    switch (tag >>> 3) {
                    case 1:
                        message.data = reader.bytes();
                        break;
                    default:
                        reader.skipType(tag & 7);
                        break;
                    }
                }
                return message;
            };
    
            /**
             * Decodes a PublicKey message from the specified reader or buffer, length delimited.
             * @function decodeDelimited
             * @memberof exonum.PublicKey
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @returns {exonum.PublicKey} PublicKey
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            PublicKey.decodeDelimited = function decodeDelimited(reader) {
                if (!(reader instanceof $Reader))
                    reader = new $Reader(reader);
                return this.decode(reader, reader.uint32());
            };
    
            /**
             * Verifies a PublicKey message.
             * @function verify
             * @memberof exonum.PublicKey
             * @static
             * @param {Object.<string,*>} message Plain object to verify
             * @returns {string|null} `null` if valid, otherwise the reason why it is not
             */
            PublicKey.verify = function verify(message) {
                if (typeof message !== "object" || message === null)
                    return "object expected";
                if (message.data != null && message.hasOwnProperty("data"))
                    if (!(message.data && typeof message.data.length === "number" || $util.isString(message.data)))
                        return "data: buffer expected";
                return null;
            };
    
            /**
             * Creates a PublicKey message from a plain object. Also converts values to their respective internal types.
             * @function fromObject
             * @memberof exonum.PublicKey
             * @static
             * @param {Object.<string,*>} object Plain object
             * @returns {exonum.PublicKey} PublicKey
             */
            PublicKey.fromObject = function fromObject(object) {
                if (object instanceof $root.exonum.PublicKey)
                    return object;
                var message = new $root.exonum.PublicKey();
                if (object.data != null)
                    if (typeof object.data === "string")
                        $util.base64.decode(object.data, message.data = $util.newBuffer($util.base64.length(object.data)), 0);
                    else if (object.data.length)
                        message.data = object.data;
                return message;
            };
    
            /**
             * Creates a plain object from a PublicKey message. Also converts values to other types if specified.
             * @function toObject
             * @memberof exonum.PublicKey
             * @static
             * @param {exonum.PublicKey} message PublicKey
             * @param {$protobuf.IConversionOptions} [options] Conversion options
             * @returns {Object.<string,*>} Plain object
             */
            PublicKey.toObject = function toObject(message, options) {
                if (!options)
                    options = {};
                var object = {};
                if (options.defaults)
                    if (options.bytes === String)
                        object.data = "";
                    else {
                        object.data = [];
                        if (options.bytes !== Array)
                            object.data = $util.newBuffer(object.data);
                    }
                if (message.data != null && message.hasOwnProperty("data"))
                    object.data = options.bytes === String ? $util.base64.encode(message.data, 0, message.data.length) : options.bytes === Array ? Array.prototype.slice.call(message.data) : message.data;
                return object;
            };
    
            /**
             * Converts this PublicKey to JSON.
             * @function toJSON
             * @memberof exonum.PublicKey
             * @instance
             * @returns {Object.<string,*>} JSON object
             */
            PublicKey.prototype.toJSON = function toJSON() {
                return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
            };
    
            return PublicKey;
        })();
    
        exonum.BitVec = (function() {
    
            /**
             * Properties of a BitVec.
             * @memberof exonum
             * @interface IBitVec
             * @property {Uint8Array|null} [data] BitVec data
             * @property {number|Long|null} [len] BitVec len
             */
    
            /**
             * Constructs a new BitVec.
             * @memberof exonum
             * @classdesc Represents a BitVec.
             * @implements IBitVec
             * @constructor
             * @param {exonum.IBitVec=} [properties] Properties to set
             */
            function BitVec(properties) {
                if (properties)
                    for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                        if (properties[keys[i]] != null)
                            this[keys[i]] = properties[keys[i]];
            }
    
            /**
             * BitVec data.
             * @member {Uint8Array} data
             * @memberof exonum.BitVec
             * @instance
             */
            BitVec.prototype.data = $util.newBuffer([]);
    
            /**
             * BitVec len.
             * @member {number|Long} len
             * @memberof exonum.BitVec
             * @instance
             */
            BitVec.prototype.len = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
            /**
             * Creates a new BitVec instance using the specified properties.
             * @function create
             * @memberof exonum.BitVec
             * @static
             * @param {exonum.IBitVec=} [properties] Properties to set
             * @returns {exonum.BitVec} BitVec instance
             */
            BitVec.create = function create(properties) {
                return new BitVec(properties);
            };
    
            /**
             * Encodes the specified BitVec message. Does not implicitly {@link exonum.BitVec.verify|verify} messages.
             * @function encode
             * @memberof exonum.BitVec
             * @static
             * @param {exonum.IBitVec} message BitVec message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            BitVec.encode = function encode(message, writer) {
                if (!writer)
                    writer = $Writer.create();
                if (message.data != null && message.hasOwnProperty("data"))
                    writer.uint32(/* id 1, wireType 2 =*/10).bytes(message.data);
                if (message.len != null && message.hasOwnProperty("len"))
                    writer.uint32(/* id 2, wireType 0 =*/16).uint64(message.len);
                return writer;
            };
    
            /**
             * Encodes the specified BitVec message, length delimited. Does not implicitly {@link exonum.BitVec.verify|verify} messages.
             * @function encodeDelimited
             * @memberof exonum.BitVec
             * @static
             * @param {exonum.IBitVec} message BitVec message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            BitVec.encodeDelimited = function encodeDelimited(message, writer) {
                return this.encode(message, writer).ldelim();
            };
    
            /**
             * Decodes a BitVec message from the specified reader or buffer.
             * @function decode
             * @memberof exonum.BitVec
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @param {number} [length] Message length if known beforehand
             * @returns {exonum.BitVec} BitVec
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            BitVec.decode = function decode(reader, length) {
                if (!(reader instanceof $Reader))
                    reader = $Reader.create(reader);
                var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.BitVec();
                while (reader.pos < end) {
                    var tag = reader.uint32();
                    switch (tag >>> 3) {
                    case 1:
                        message.data = reader.bytes();
                        break;
                    case 2:
                        message.len = reader.uint64();
                        break;
                    default:
                        reader.skipType(tag & 7);
                        break;
                    }
                }
                return message;
            };
    
            /**
             * Decodes a BitVec message from the specified reader or buffer, length delimited.
             * @function decodeDelimited
             * @memberof exonum.BitVec
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @returns {exonum.BitVec} BitVec
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            BitVec.decodeDelimited = function decodeDelimited(reader) {
                if (!(reader instanceof $Reader))
                    reader = new $Reader(reader);
                return this.decode(reader, reader.uint32());
            };
    
            /**
             * Verifies a BitVec message.
             * @function verify
             * @memberof exonum.BitVec
             * @static
             * @param {Object.<string,*>} message Plain object to verify
             * @returns {string|null} `null` if valid, otherwise the reason why it is not
             */
            BitVec.verify = function verify(message) {
                if (typeof message !== "object" || message === null)
                    return "object expected";
                if (message.data != null && message.hasOwnProperty("data"))
                    if (!(message.data && typeof message.data.length === "number" || $util.isString(message.data)))
                        return "data: buffer expected";
                if (message.len != null && message.hasOwnProperty("len"))
                    if (!$util.isInteger(message.len) && !(message.len && $util.isInteger(message.len.low) && $util.isInteger(message.len.high)))
                        return "len: integer|Long expected";
                return null;
            };
    
            /**
             * Creates a BitVec message from a plain object. Also converts values to their respective internal types.
             * @function fromObject
             * @memberof exonum.BitVec
             * @static
             * @param {Object.<string,*>} object Plain object
             * @returns {exonum.BitVec} BitVec
             */
            BitVec.fromObject = function fromObject(object) {
                if (object instanceof $root.exonum.BitVec)
                    return object;
                var message = new $root.exonum.BitVec();
                if (object.data != null)
                    if (typeof object.data === "string")
                        $util.base64.decode(object.data, message.data = $util.newBuffer($util.base64.length(object.data)), 0);
                    else if (object.data.length)
                        message.data = object.data;
                if (object.len != null)
                    if ($util.Long)
                        (message.len = $util.Long.fromValue(object.len)).unsigned = true;
                    else if (typeof object.len === "string")
                        message.len = parseInt(object.len, 10);
                    else if (typeof object.len === "number")
                        message.len = object.len;
                    else if (typeof object.len === "object")
                        message.len = new $util.LongBits(object.len.low >>> 0, object.len.high >>> 0).toNumber(true);
                return message;
            };
    
            /**
             * Creates a plain object from a BitVec message. Also converts values to other types if specified.
             * @function toObject
             * @memberof exonum.BitVec
             * @static
             * @param {exonum.BitVec} message BitVec
             * @param {$protobuf.IConversionOptions} [options] Conversion options
             * @returns {Object.<string,*>} Plain object
             */
            BitVec.toObject = function toObject(message, options) {
                if (!options)
                    options = {};
                var object = {};
                if (options.defaults) {
                    if (options.bytes === String)
                        object.data = "";
                    else {
                        object.data = [];
                        if (options.bytes !== Array)
                            object.data = $util.newBuffer(object.data);
                    }
                    if ($util.Long) {
                        var long = new $util.Long(0, 0, true);
                        object.len = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                    } else
                        object.len = options.longs === String ? "0" : 0;
                }
                if (message.data != null && message.hasOwnProperty("data"))
                    object.data = options.bytes === String ? $util.base64.encode(message.data, 0, message.data.length) : options.bytes === Array ? Array.prototype.slice.call(message.data) : message.data;
                if (message.len != null && message.hasOwnProperty("len"))
                    if (typeof message.len === "number")
                        object.len = options.longs === String ? String(message.len) : message.len;
                    else
                        object.len = options.longs === String ? $util.Long.prototype.toString.call(message.len) : options.longs === Number ? new $util.LongBits(message.len.low >>> 0, message.len.high >>> 0).toNumber(true) : message.len;
                return object;
            };
    
            /**
             * Converts this BitVec to JSON.
             * @function toJSON
             * @memberof exonum.BitVec
             * @instance
             * @returns {Object.<string,*>} JSON object
             */
            BitVec.prototype.toJSON = function toJSON() {
                return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
            };
    
            return BitVec;
        })();
    
        exonum.Block = (function() {
    
            /**
             * Properties of a Block.
             * @memberof exonum
             * @interface IBlock
             * @property {number|null} [proposerId] Block proposerId
             * @property {number|Long|null} [height] Block height
             * @property {number|null} [txCount] Block txCount
             * @property {exonum.IHash|null} [prevHash] Block prevHash
             * @property {exonum.IHash|null} [txHash] Block txHash
             * @property {exonum.IHash|null} [stateHash] Block stateHash
             */
    
            /**
             * Constructs a new Block.
             * @memberof exonum
             * @classdesc Represents a Block.
             * @implements IBlock
             * @constructor
             * @param {exonum.IBlock=} [properties] Properties to set
             */
            function Block(properties) {
                if (properties)
                    for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                        if (properties[keys[i]] != null)
                            this[keys[i]] = properties[keys[i]];
            }
    
            /**
             * Block proposerId.
             * @member {number} proposerId
             * @memberof exonum.Block
             * @instance
             */
            Block.prototype.proposerId = 0;
    
            /**
             * Block height.
             * @member {number|Long} height
             * @memberof exonum.Block
             * @instance
             */
            Block.prototype.height = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
            /**
             * Block txCount.
             * @member {number} txCount
             * @memberof exonum.Block
             * @instance
             */
            Block.prototype.txCount = 0;
    
            /**
             * Block prevHash.
             * @member {exonum.IHash|null|undefined} prevHash
             * @memberof exonum.Block
             * @instance
             */
            Block.prototype.prevHash = null;
    
            /**
             * Block txHash.
             * @member {exonum.IHash|null|undefined} txHash
             * @memberof exonum.Block
             * @instance
             */
            Block.prototype.txHash = null;
    
            /**
             * Block stateHash.
             * @member {exonum.IHash|null|undefined} stateHash
             * @memberof exonum.Block
             * @instance
             */
            Block.prototype.stateHash = null;
    
            /**
             * Creates a new Block instance using the specified properties.
             * @function create
             * @memberof exonum.Block
             * @static
             * @param {exonum.IBlock=} [properties] Properties to set
             * @returns {exonum.Block} Block instance
             */
            Block.create = function create(properties) {
                return new Block(properties);
            };
    
            /**
             * Encodes the specified Block message. Does not implicitly {@link exonum.Block.verify|verify} messages.
             * @function encode
             * @memberof exonum.Block
             * @static
             * @param {exonum.IBlock} message Block message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            Block.encode = function encode(message, writer) {
                if (!writer)
                    writer = $Writer.create();
                if (message.proposerId != null && message.hasOwnProperty("proposerId"))
                    writer.uint32(/* id 1, wireType 0 =*/8).uint32(message.proposerId);
                if (message.height != null && message.hasOwnProperty("height"))
                    writer.uint32(/* id 2, wireType 0 =*/16).uint64(message.height);
                if (message.txCount != null && message.hasOwnProperty("txCount"))
                    writer.uint32(/* id 3, wireType 0 =*/24).uint32(message.txCount);
                if (message.prevHash != null && message.hasOwnProperty("prevHash"))
                    $root.exonum.Hash.encode(message.prevHash, writer.uint32(/* id 4, wireType 2 =*/34).fork()).ldelim();
                if (message.txHash != null && message.hasOwnProperty("txHash"))
                    $root.exonum.Hash.encode(message.txHash, writer.uint32(/* id 5, wireType 2 =*/42).fork()).ldelim();
                if (message.stateHash != null && message.hasOwnProperty("stateHash"))
                    $root.exonum.Hash.encode(message.stateHash, writer.uint32(/* id 6, wireType 2 =*/50).fork()).ldelim();
                return writer;
            };
    
            /**
             * Encodes the specified Block message, length delimited. Does not implicitly {@link exonum.Block.verify|verify} messages.
             * @function encodeDelimited
             * @memberof exonum.Block
             * @static
             * @param {exonum.IBlock} message Block message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            Block.encodeDelimited = function encodeDelimited(message, writer) {
                return this.encode(message, writer).ldelim();
            };
    
            /**
             * Decodes a Block message from the specified reader or buffer.
             * @function decode
             * @memberof exonum.Block
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @param {number} [length] Message length if known beforehand
             * @returns {exonum.Block} Block
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            Block.decode = function decode(reader, length) {
                if (!(reader instanceof $Reader))
                    reader = $Reader.create(reader);
                var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.Block();
                while (reader.pos < end) {
                    var tag = reader.uint32();
                    switch (tag >>> 3) {
                    case 1:
                        message.proposerId = reader.uint32();
                        break;
                    case 2:
                        message.height = reader.uint64();
                        break;
                    case 3:
                        message.txCount = reader.uint32();
                        break;
                    case 4:
                        message.prevHash = $root.exonum.Hash.decode(reader, reader.uint32());
                        break;
                    case 5:
                        message.txHash = $root.exonum.Hash.decode(reader, reader.uint32());
                        break;
                    case 6:
                        message.stateHash = $root.exonum.Hash.decode(reader, reader.uint32());
                        break;
                    default:
                        reader.skipType(tag & 7);
                        break;
                    }
                }
                return message;
            };
    
            /**
             * Decodes a Block message from the specified reader or buffer, length delimited.
             * @function decodeDelimited
             * @memberof exonum.Block
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @returns {exonum.Block} Block
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            Block.decodeDelimited = function decodeDelimited(reader) {
                if (!(reader instanceof $Reader))
                    reader = new $Reader(reader);
                return this.decode(reader, reader.uint32());
            };
    
            /**
             * Verifies a Block message.
             * @function verify
             * @memberof exonum.Block
             * @static
             * @param {Object.<string,*>} message Plain object to verify
             * @returns {string|null} `null` if valid, otherwise the reason why it is not
             */
            Block.verify = function verify(message) {
                if (typeof message !== "object" || message === null)
                    return "object expected";
                if (message.proposerId != null && message.hasOwnProperty("proposerId"))
                    if (!$util.isInteger(message.proposerId))
                        return "proposerId: integer expected";
                if (message.height != null && message.hasOwnProperty("height"))
                    if (!$util.isInteger(message.height) && !(message.height && $util.isInteger(message.height.low) && $util.isInteger(message.height.high)))
                        return "height: integer|Long expected";
                if (message.txCount != null && message.hasOwnProperty("txCount"))
                    if (!$util.isInteger(message.txCount))
                        return "txCount: integer expected";
                if (message.prevHash != null && message.hasOwnProperty("prevHash")) {
                    var error = $root.exonum.Hash.verify(message.prevHash);
                    if (error)
                        return "prevHash." + error;
                }
                if (message.txHash != null && message.hasOwnProperty("txHash")) {
                    var error = $root.exonum.Hash.verify(message.txHash);
                    if (error)
                        return "txHash." + error;
                }
                if (message.stateHash != null && message.hasOwnProperty("stateHash")) {
                    var error = $root.exonum.Hash.verify(message.stateHash);
                    if (error)
                        return "stateHash." + error;
                }
                return null;
            };
    
            /**
             * Creates a Block message from a plain object. Also converts values to their respective internal types.
             * @function fromObject
             * @memberof exonum.Block
             * @static
             * @param {Object.<string,*>} object Plain object
             * @returns {exonum.Block} Block
             */
            Block.fromObject = function fromObject(object) {
                if (object instanceof $root.exonum.Block)
                    return object;
                var message = new $root.exonum.Block();
                if (object.proposerId != null)
                    message.proposerId = object.proposerId >>> 0;
                if (object.height != null)
                    if ($util.Long)
                        (message.height = $util.Long.fromValue(object.height)).unsigned = true;
                    else if (typeof object.height === "string")
                        message.height = parseInt(object.height, 10);
                    else if (typeof object.height === "number")
                        message.height = object.height;
                    else if (typeof object.height === "object")
                        message.height = new $util.LongBits(object.height.low >>> 0, object.height.high >>> 0).toNumber(true);
                if (object.txCount != null)
                    message.txCount = object.txCount >>> 0;
                if (object.prevHash != null) {
                    if (typeof object.prevHash !== "object")
                        throw TypeError(".exonum.Block.prevHash: object expected");
                    message.prevHash = $root.exonum.Hash.fromObject(object.prevHash);
                }
                if (object.txHash != null) {
                    if (typeof object.txHash !== "object")
                        throw TypeError(".exonum.Block.txHash: object expected");
                    message.txHash = $root.exonum.Hash.fromObject(object.txHash);
                }
                if (object.stateHash != null) {
                    if (typeof object.stateHash !== "object")
                        throw TypeError(".exonum.Block.stateHash: object expected");
                    message.stateHash = $root.exonum.Hash.fromObject(object.stateHash);
                }
                return message;
            };
    
            /**
             * Creates a plain object from a Block message. Also converts values to other types if specified.
             * @function toObject
             * @memberof exonum.Block
             * @static
             * @param {exonum.Block} message Block
             * @param {$protobuf.IConversionOptions} [options] Conversion options
             * @returns {Object.<string,*>} Plain object
             */
            Block.toObject = function toObject(message, options) {
                if (!options)
                    options = {};
                var object = {};
                if (options.defaults) {
                    object.proposerId = 0;
                    if ($util.Long) {
                        var long = new $util.Long(0, 0, true);
                        object.height = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                    } else
                        object.height = options.longs === String ? "0" : 0;
                    object.txCount = 0;
                    object.prevHash = null;
                    object.txHash = null;
                    object.stateHash = null;
                }
                if (message.proposerId != null && message.hasOwnProperty("proposerId"))
                    object.proposerId = message.proposerId;
                if (message.height != null && message.hasOwnProperty("height"))
                    if (typeof message.height === "number")
                        object.height = options.longs === String ? String(message.height) : message.height;
                    else
                        object.height = options.longs === String ? $util.Long.prototype.toString.call(message.height) : options.longs === Number ? new $util.LongBits(message.height.low >>> 0, message.height.high >>> 0).toNumber(true) : message.height;
                if (message.txCount != null && message.hasOwnProperty("txCount"))
                    object.txCount = message.txCount;
                if (message.prevHash != null && message.hasOwnProperty("prevHash"))
                    object.prevHash = $root.exonum.Hash.toObject(message.prevHash, options);
                if (message.txHash != null && message.hasOwnProperty("txHash"))
                    object.txHash = $root.exonum.Hash.toObject(message.txHash, options);
                if (message.stateHash != null && message.hasOwnProperty("stateHash"))
                    object.stateHash = $root.exonum.Hash.toObject(message.stateHash, options);
                return object;
            };
    
            /**
             * Converts this Block to JSON.
             * @function toJSON
             * @memberof exonum.Block
             * @instance
             * @returns {Object.<string,*>} JSON object
             */
            Block.prototype.toJSON = function toJSON() {
                return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
            };
    
            return Block;
        })();
    
        exonum.ConfigReference = (function() {
    
            /**
             * Properties of a ConfigReference.
             * @memberof exonum
             * @interface IConfigReference
             * @property {number|Long|null} [actualFrom] ConfigReference actualFrom
             * @property {exonum.IHash|null} [cfgHash] ConfigReference cfgHash
             */
    
            /**
             * Constructs a new ConfigReference.
             * @memberof exonum
             * @classdesc Represents a ConfigReference.
             * @implements IConfigReference
             * @constructor
             * @param {exonum.IConfigReference=} [properties] Properties to set
             */
            function ConfigReference(properties) {
                if (properties)
                    for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                        if (properties[keys[i]] != null)
                            this[keys[i]] = properties[keys[i]];
            }
    
            /**
             * ConfigReference actualFrom.
             * @member {number|Long} actualFrom
             * @memberof exonum.ConfigReference
             * @instance
             */
            ConfigReference.prototype.actualFrom = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
            /**
             * ConfigReference cfgHash.
             * @member {exonum.IHash|null|undefined} cfgHash
             * @memberof exonum.ConfigReference
             * @instance
             */
            ConfigReference.prototype.cfgHash = null;
    
            /**
             * Creates a new ConfigReference instance using the specified properties.
             * @function create
             * @memberof exonum.ConfigReference
             * @static
             * @param {exonum.IConfigReference=} [properties] Properties to set
             * @returns {exonum.ConfigReference} ConfigReference instance
             */
            ConfigReference.create = function create(properties) {
                return new ConfigReference(properties);
            };
    
            /**
             * Encodes the specified ConfigReference message. Does not implicitly {@link exonum.ConfigReference.verify|verify} messages.
             * @function encode
             * @memberof exonum.ConfigReference
             * @static
             * @param {exonum.IConfigReference} message ConfigReference message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            ConfigReference.encode = function encode(message, writer) {
                if (!writer)
                    writer = $Writer.create();
                if (message.actualFrom != null && message.hasOwnProperty("actualFrom"))
                    writer.uint32(/* id 1, wireType 0 =*/8).uint64(message.actualFrom);
                if (message.cfgHash != null && message.hasOwnProperty("cfgHash"))
                    $root.exonum.Hash.encode(message.cfgHash, writer.uint32(/* id 2, wireType 2 =*/18).fork()).ldelim();
                return writer;
            };
    
            /**
             * Encodes the specified ConfigReference message, length delimited. Does not implicitly {@link exonum.ConfigReference.verify|verify} messages.
             * @function encodeDelimited
             * @memberof exonum.ConfigReference
             * @static
             * @param {exonum.IConfigReference} message ConfigReference message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            ConfigReference.encodeDelimited = function encodeDelimited(message, writer) {
                return this.encode(message, writer).ldelim();
            };
    
            /**
             * Decodes a ConfigReference message from the specified reader or buffer.
             * @function decode
             * @memberof exonum.ConfigReference
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @param {number} [length] Message length if known beforehand
             * @returns {exonum.ConfigReference} ConfigReference
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            ConfigReference.decode = function decode(reader, length) {
                if (!(reader instanceof $Reader))
                    reader = $Reader.create(reader);
                var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.ConfigReference();
                while (reader.pos < end) {
                    var tag = reader.uint32();
                    switch (tag >>> 3) {
                    case 1:
                        message.actualFrom = reader.uint64();
                        break;
                    case 2:
                        message.cfgHash = $root.exonum.Hash.decode(reader, reader.uint32());
                        break;
                    default:
                        reader.skipType(tag & 7);
                        break;
                    }
                }
                return message;
            };
    
            /**
             * Decodes a ConfigReference message from the specified reader or buffer, length delimited.
             * @function decodeDelimited
             * @memberof exonum.ConfigReference
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @returns {exonum.ConfigReference} ConfigReference
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            ConfigReference.decodeDelimited = function decodeDelimited(reader) {
                if (!(reader instanceof $Reader))
                    reader = new $Reader(reader);
                return this.decode(reader, reader.uint32());
            };
    
            /**
             * Verifies a ConfigReference message.
             * @function verify
             * @memberof exonum.ConfigReference
             * @static
             * @param {Object.<string,*>} message Plain object to verify
             * @returns {string|null} `null` if valid, otherwise the reason why it is not
             */
            ConfigReference.verify = function verify(message) {
                if (typeof message !== "object" || message === null)
                    return "object expected";
                if (message.actualFrom != null && message.hasOwnProperty("actualFrom"))
                    if (!$util.isInteger(message.actualFrom) && !(message.actualFrom && $util.isInteger(message.actualFrom.low) && $util.isInteger(message.actualFrom.high)))
                        return "actualFrom: integer|Long expected";
                if (message.cfgHash != null && message.hasOwnProperty("cfgHash")) {
                    var error = $root.exonum.Hash.verify(message.cfgHash);
                    if (error)
                        return "cfgHash." + error;
                }
                return null;
            };
    
            /**
             * Creates a ConfigReference message from a plain object. Also converts values to their respective internal types.
             * @function fromObject
             * @memberof exonum.ConfigReference
             * @static
             * @param {Object.<string,*>} object Plain object
             * @returns {exonum.ConfigReference} ConfigReference
             */
            ConfigReference.fromObject = function fromObject(object) {
                if (object instanceof $root.exonum.ConfigReference)
                    return object;
                var message = new $root.exonum.ConfigReference();
                if (object.actualFrom != null)
                    if ($util.Long)
                        (message.actualFrom = $util.Long.fromValue(object.actualFrom)).unsigned = true;
                    else if (typeof object.actualFrom === "string")
                        message.actualFrom = parseInt(object.actualFrom, 10);
                    else if (typeof object.actualFrom === "number")
                        message.actualFrom = object.actualFrom;
                    else if (typeof object.actualFrom === "object")
                        message.actualFrom = new $util.LongBits(object.actualFrom.low >>> 0, object.actualFrom.high >>> 0).toNumber(true);
                if (object.cfgHash != null) {
                    if (typeof object.cfgHash !== "object")
                        throw TypeError(".exonum.ConfigReference.cfgHash: object expected");
                    message.cfgHash = $root.exonum.Hash.fromObject(object.cfgHash);
                }
                return message;
            };
    
            /**
             * Creates a plain object from a ConfigReference message. Also converts values to other types if specified.
             * @function toObject
             * @memberof exonum.ConfigReference
             * @static
             * @param {exonum.ConfigReference} message ConfigReference
             * @param {$protobuf.IConversionOptions} [options] Conversion options
             * @returns {Object.<string,*>} Plain object
             */
            ConfigReference.toObject = function toObject(message, options) {
                if (!options)
                    options = {};
                var object = {};
                if (options.defaults) {
                    if ($util.Long) {
                        var long = new $util.Long(0, 0, true);
                        object.actualFrom = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                    } else
                        object.actualFrom = options.longs === String ? "0" : 0;
                    object.cfgHash = null;
                }
                if (message.actualFrom != null && message.hasOwnProperty("actualFrom"))
                    if (typeof message.actualFrom === "number")
                        object.actualFrom = options.longs === String ? String(message.actualFrom) : message.actualFrom;
                    else
                        object.actualFrom = options.longs === String ? $util.Long.prototype.toString.call(message.actualFrom) : options.longs === Number ? new $util.LongBits(message.actualFrom.low >>> 0, message.actualFrom.high >>> 0).toNumber(true) : message.actualFrom;
                if (message.cfgHash != null && message.hasOwnProperty("cfgHash"))
                    object.cfgHash = $root.exonum.Hash.toObject(message.cfgHash, options);
                return object;
            };
    
            /**
             * Converts this ConfigReference to JSON.
             * @function toJSON
             * @memberof exonum.ConfigReference
             * @instance
             * @returns {Object.<string,*>} JSON object
             */
            ConfigReference.prototype.toJSON = function toJSON() {
                return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
            };
    
            return ConfigReference;
        })();
    
        exonum.TxLocation = (function() {
    
            /**
             * Properties of a TxLocation.
             * @memberof exonum
             * @interface ITxLocation
             * @property {number|Long|null} [blockHeight] TxLocation blockHeight
             * @property {number|Long|null} [positionInBlock] TxLocation positionInBlock
             */
    
            /**
             * Constructs a new TxLocation.
             * @memberof exonum
             * @classdesc Represents a TxLocation.
             * @implements ITxLocation
             * @constructor
             * @param {exonum.ITxLocation=} [properties] Properties to set
             */
            function TxLocation(properties) {
                if (properties)
                    for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                        if (properties[keys[i]] != null)
                            this[keys[i]] = properties[keys[i]];
            }
    
            /**
             * TxLocation blockHeight.
             * @member {number|Long} blockHeight
             * @memberof exonum.TxLocation
             * @instance
             */
            TxLocation.prototype.blockHeight = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
            /**
             * TxLocation positionInBlock.
             * @member {number|Long} positionInBlock
             * @memberof exonum.TxLocation
             * @instance
             */
            TxLocation.prototype.positionInBlock = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
            /**
             * Creates a new TxLocation instance using the specified properties.
             * @function create
             * @memberof exonum.TxLocation
             * @static
             * @param {exonum.ITxLocation=} [properties] Properties to set
             * @returns {exonum.TxLocation} TxLocation instance
             */
            TxLocation.create = function create(properties) {
                return new TxLocation(properties);
            };
    
            /**
             * Encodes the specified TxLocation message. Does not implicitly {@link exonum.TxLocation.verify|verify} messages.
             * @function encode
             * @memberof exonum.TxLocation
             * @static
             * @param {exonum.ITxLocation} message TxLocation message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            TxLocation.encode = function encode(message, writer) {
                if (!writer)
                    writer = $Writer.create();
                if (message.blockHeight != null && message.hasOwnProperty("blockHeight"))
                    writer.uint32(/* id 1, wireType 0 =*/8).uint64(message.blockHeight);
                if (message.positionInBlock != null && message.hasOwnProperty("positionInBlock"))
                    writer.uint32(/* id 2, wireType 0 =*/16).uint64(message.positionInBlock);
                return writer;
            };
    
            /**
             * Encodes the specified TxLocation message, length delimited. Does not implicitly {@link exonum.TxLocation.verify|verify} messages.
             * @function encodeDelimited
             * @memberof exonum.TxLocation
             * @static
             * @param {exonum.ITxLocation} message TxLocation message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            TxLocation.encodeDelimited = function encodeDelimited(message, writer) {
                return this.encode(message, writer).ldelim();
            };
    
            /**
             * Decodes a TxLocation message from the specified reader or buffer.
             * @function decode
             * @memberof exonum.TxLocation
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @param {number} [length] Message length if known beforehand
             * @returns {exonum.TxLocation} TxLocation
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            TxLocation.decode = function decode(reader, length) {
                if (!(reader instanceof $Reader))
                    reader = $Reader.create(reader);
                var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.TxLocation();
                while (reader.pos < end) {
                    var tag = reader.uint32();
                    switch (tag >>> 3) {
                    case 1:
                        message.blockHeight = reader.uint64();
                        break;
                    case 2:
                        message.positionInBlock = reader.uint64();
                        break;
                    default:
                        reader.skipType(tag & 7);
                        break;
                    }
                }
                return message;
            };
    
            /**
             * Decodes a TxLocation message from the specified reader or buffer, length delimited.
             * @function decodeDelimited
             * @memberof exonum.TxLocation
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @returns {exonum.TxLocation} TxLocation
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            TxLocation.decodeDelimited = function decodeDelimited(reader) {
                if (!(reader instanceof $Reader))
                    reader = new $Reader(reader);
                return this.decode(reader, reader.uint32());
            };
    
            /**
             * Verifies a TxLocation message.
             * @function verify
             * @memberof exonum.TxLocation
             * @static
             * @param {Object.<string,*>} message Plain object to verify
             * @returns {string|null} `null` if valid, otherwise the reason why it is not
             */
            TxLocation.verify = function verify(message) {
                if (typeof message !== "object" || message === null)
                    return "object expected";
                if (message.blockHeight != null && message.hasOwnProperty("blockHeight"))
                    if (!$util.isInteger(message.blockHeight) && !(message.blockHeight && $util.isInteger(message.blockHeight.low) && $util.isInteger(message.blockHeight.high)))
                        return "blockHeight: integer|Long expected";
                if (message.positionInBlock != null && message.hasOwnProperty("positionInBlock"))
                    if (!$util.isInteger(message.positionInBlock) && !(message.positionInBlock && $util.isInteger(message.positionInBlock.low) && $util.isInteger(message.positionInBlock.high)))
                        return "positionInBlock: integer|Long expected";
                return null;
            };
    
            /**
             * Creates a TxLocation message from a plain object. Also converts values to their respective internal types.
             * @function fromObject
             * @memberof exonum.TxLocation
             * @static
             * @param {Object.<string,*>} object Plain object
             * @returns {exonum.TxLocation} TxLocation
             */
            TxLocation.fromObject = function fromObject(object) {
                if (object instanceof $root.exonum.TxLocation)
                    return object;
                var message = new $root.exonum.TxLocation();
                if (object.blockHeight != null)
                    if ($util.Long)
                        (message.blockHeight = $util.Long.fromValue(object.blockHeight)).unsigned = true;
                    else if (typeof object.blockHeight === "string")
                        message.blockHeight = parseInt(object.blockHeight, 10);
                    else if (typeof object.blockHeight === "number")
                        message.blockHeight = object.blockHeight;
                    else if (typeof object.blockHeight === "object")
                        message.blockHeight = new $util.LongBits(object.blockHeight.low >>> 0, object.blockHeight.high >>> 0).toNumber(true);
                if (object.positionInBlock != null)
                    if ($util.Long)
                        (message.positionInBlock = $util.Long.fromValue(object.positionInBlock)).unsigned = true;
                    else if (typeof object.positionInBlock === "string")
                        message.positionInBlock = parseInt(object.positionInBlock, 10);
                    else if (typeof object.positionInBlock === "number")
                        message.positionInBlock = object.positionInBlock;
                    else if (typeof object.positionInBlock === "object")
                        message.positionInBlock = new $util.LongBits(object.positionInBlock.low >>> 0, object.positionInBlock.high >>> 0).toNumber(true);
                return message;
            };
    
            /**
             * Creates a plain object from a TxLocation message. Also converts values to other types if specified.
             * @function toObject
             * @memberof exonum.TxLocation
             * @static
             * @param {exonum.TxLocation} message TxLocation
             * @param {$protobuf.IConversionOptions} [options] Conversion options
             * @returns {Object.<string,*>} Plain object
             */
            TxLocation.toObject = function toObject(message, options) {
                if (!options)
                    options = {};
                var object = {};
                if (options.defaults) {
                    if ($util.Long) {
                        var long = new $util.Long(0, 0, true);
                        object.blockHeight = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                    } else
                        object.blockHeight = options.longs === String ? "0" : 0;
                    if ($util.Long) {
                        var long = new $util.Long(0, 0, true);
                        object.positionInBlock = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                    } else
                        object.positionInBlock = options.longs === String ? "0" : 0;
                }
                if (message.blockHeight != null && message.hasOwnProperty("blockHeight"))
                    if (typeof message.blockHeight === "number")
                        object.blockHeight = options.longs === String ? String(message.blockHeight) : message.blockHeight;
                    else
                        object.blockHeight = options.longs === String ? $util.Long.prototype.toString.call(message.blockHeight) : options.longs === Number ? new $util.LongBits(message.blockHeight.low >>> 0, message.blockHeight.high >>> 0).toNumber(true) : message.blockHeight;
                if (message.positionInBlock != null && message.hasOwnProperty("positionInBlock"))
                    if (typeof message.positionInBlock === "number")
                        object.positionInBlock = options.longs === String ? String(message.positionInBlock) : message.positionInBlock;
                    else
                        object.positionInBlock = options.longs === String ? $util.Long.prototype.toString.call(message.positionInBlock) : options.longs === Number ? new $util.LongBits(message.positionInBlock.low >>> 0, message.positionInBlock.high >>> 0).toNumber(true) : message.positionInBlock;
                return object;
            };
    
            /**
             * Converts this TxLocation to JSON.
             * @function toJSON
             * @memberof exonum.TxLocation
             * @instance
             * @returns {Object.<string,*>} JSON object
             */
            TxLocation.prototype.toJSON = function toJSON() {
                return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
            };
    
            return TxLocation;
        })();
    
        exonum.TransactionResult = (function() {
    
            /**
             * Properties of a TransactionResult.
             * @memberof exonum
             * @interface ITransactionResult
             * @property {number|null} [status] TransactionResult status
             * @property {string|null} [description] TransactionResult description
             */
    
            /**
             * Constructs a new TransactionResult.
             * @memberof exonum
             * @classdesc Represents a TransactionResult.
             * @implements ITransactionResult
             * @constructor
             * @param {exonum.ITransactionResult=} [properties] Properties to set
             */
            function TransactionResult(properties) {
                if (properties)
                    for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                        if (properties[keys[i]] != null)
                            this[keys[i]] = properties[keys[i]];
            }
    
            /**
             * TransactionResult status.
             * @member {number} status
             * @memberof exonum.TransactionResult
             * @instance
             */
            TransactionResult.prototype.status = 0;
    
            /**
             * TransactionResult description.
             * @member {string} description
             * @memberof exonum.TransactionResult
             * @instance
             */
            TransactionResult.prototype.description = "";
    
            /**
             * Creates a new TransactionResult instance using the specified properties.
             * @function create
             * @memberof exonum.TransactionResult
             * @static
             * @param {exonum.ITransactionResult=} [properties] Properties to set
             * @returns {exonum.TransactionResult} TransactionResult instance
             */
            TransactionResult.create = function create(properties) {
                return new TransactionResult(properties);
            };
    
            /**
             * Encodes the specified TransactionResult message. Does not implicitly {@link exonum.TransactionResult.verify|verify} messages.
             * @function encode
             * @memberof exonum.TransactionResult
             * @static
             * @param {exonum.ITransactionResult} message TransactionResult message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            TransactionResult.encode = function encode(message, writer) {
                if (!writer)
                    writer = $Writer.create();
                if (message.status != null && message.hasOwnProperty("status"))
                    writer.uint32(/* id 1, wireType 0 =*/8).uint32(message.status);
                if (message.description != null && message.hasOwnProperty("description"))
                    writer.uint32(/* id 2, wireType 2 =*/18).string(message.description);
                return writer;
            };
    
            /**
             * Encodes the specified TransactionResult message, length delimited. Does not implicitly {@link exonum.TransactionResult.verify|verify} messages.
             * @function encodeDelimited
             * @memberof exonum.TransactionResult
             * @static
             * @param {exonum.ITransactionResult} message TransactionResult message or plain object to encode
             * @param {$protobuf.Writer} [writer] Writer to encode to
             * @returns {$protobuf.Writer} Writer
             */
            TransactionResult.encodeDelimited = function encodeDelimited(message, writer) {
                return this.encode(message, writer).ldelim();
            };
    
            /**
             * Decodes a TransactionResult message from the specified reader or buffer.
             * @function decode
             * @memberof exonum.TransactionResult
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @param {number} [length] Message length if known beforehand
             * @returns {exonum.TransactionResult} TransactionResult
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            TransactionResult.decode = function decode(reader, length) {
                if (!(reader instanceof $Reader))
                    reader = $Reader.create(reader);
                var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.TransactionResult();
                while (reader.pos < end) {
                    var tag = reader.uint32();
                    switch (tag >>> 3) {
                    case 1:
                        message.status = reader.uint32();
                        break;
                    case 2:
                        message.description = reader.string();
                        break;
                    default:
                        reader.skipType(tag & 7);
                        break;
                    }
                }
                return message;
            };
    
            /**
             * Decodes a TransactionResult message from the specified reader or buffer, length delimited.
             * @function decodeDelimited
             * @memberof exonum.TransactionResult
             * @static
             * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
             * @returns {exonum.TransactionResult} TransactionResult
             * @throws {Error} If the payload is not a reader or valid buffer
             * @throws {$protobuf.util.ProtocolError} If required fields are missing
             */
            TransactionResult.decodeDelimited = function decodeDelimited(reader) {
                if (!(reader instanceof $Reader))
                    reader = new $Reader(reader);
                return this.decode(reader, reader.uint32());
            };
    
            /**
             * Verifies a TransactionResult message.
             * @function verify
             * @memberof exonum.TransactionResult
             * @static
             * @param {Object.<string,*>} message Plain object to verify
             * @returns {string|null} `null` if valid, otherwise the reason why it is not
             */
            TransactionResult.verify = function verify(message) {
                if (typeof message !== "object" || message === null)
                    return "object expected";
                if (message.status != null && message.hasOwnProperty("status"))
                    if (!$util.isInteger(message.status))
                        return "status: integer expected";
                if (message.description != null && message.hasOwnProperty("description"))
                    if (!$util.isString(message.description))
                        return "description: string expected";
                return null;
            };
    
            /**
             * Creates a TransactionResult message from a plain object. Also converts values to their respective internal types.
             * @function fromObject
             * @memberof exonum.TransactionResult
             * @static
             * @param {Object.<string,*>} object Plain object
             * @returns {exonum.TransactionResult} TransactionResult
             */
            TransactionResult.fromObject = function fromObject(object) {
                if (object instanceof $root.exonum.TransactionResult)
                    return object;
                var message = new $root.exonum.TransactionResult();
                if (object.status != null)
                    message.status = object.status >>> 0;
                if (object.description != null)
                    message.description = String(object.description);
                return message;
            };
    
            /**
             * Creates a plain object from a TransactionResult message. Also converts values to other types if specified.
             * @function toObject
             * @memberof exonum.TransactionResult
             * @static
             * @param {exonum.TransactionResult} message TransactionResult
             * @param {$protobuf.IConversionOptions} [options] Conversion options
             * @returns {Object.<string,*>} Plain object
             */
            TransactionResult.toObject = function toObject(message, options) {
                if (!options)
                    options = {};
                var object = {};
                if (options.defaults) {
                    object.status = 0;
                    object.description = "";
                }
                if (message.status != null && message.hasOwnProperty("status"))
                    object.status = message.status;
                if (message.description != null && message.hasOwnProperty("description"))
                    object.description = message.description;
                return object;
            };
    
            /**
             * Converts this TransactionResult to JSON.
             * @function toJSON
             * @memberof exonum.TransactionResult
             * @instance
             * @returns {Object.<string,*>} JSON object
             */
            TransactionResult.prototype.toJSON = function toJSON() {
                return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
            };
    
            return TransactionResult;
        })();
    
        exonum.examples = (function() {
    
            /**
             * Namespace examples.
             * @memberof exonum
             * @namespace
             */
            var examples = {};
    
            examples.cryptocurrency_advanced = (function() {
    
                /**
                 * Namespace cryptocurrency_advanced.
                 * @memberof exonum.examples
                 * @namespace
                 */
                var cryptocurrency_advanced = {};
    
                cryptocurrency_advanced.Transfer = (function() {
    
                    /**
                     * Properties of a Transfer.
                     * @memberof exonum.examples.cryptocurrency_advanced
                     * @interface ITransfer
                     * @property {exonum.IPublicKey|null} [to] Transfer to
                     * @property {number|Long|null} [amount] Transfer amount
                     * @property {number|Long|null} [seed] Transfer seed
                     */
    
                    /**
                     * Constructs a new Transfer.
                     * @memberof exonum.examples.cryptocurrency_advanced
                     * @classdesc Transfer `amount` of the currency from one wallet to another.
                     * @implements ITransfer
                     * @constructor
                     * @param {exonum.examples.cryptocurrency_advanced.ITransfer=} [properties] Properties to set
                     */
                    function Transfer(properties) {
                        if (properties)
                            for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                                if (properties[keys[i]] != null)
                                    this[keys[i]] = properties[keys[i]];
                    }
    
                    /**
                     * Transfer to.
                     * @member {exonum.IPublicKey|null|undefined} to
                     * @memberof exonum.examples.cryptocurrency_advanced.Transfer
                     * @instance
                     */
                    Transfer.prototype.to = null;
    
                    /**
                     * Transfer amount.
                     * @member {number|Long} amount
                     * @memberof exonum.examples.cryptocurrency_advanced.Transfer
                     * @instance
                     */
                    Transfer.prototype.amount = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
                    /**
                     * Transfer seed.
                     * @member {number|Long} seed
                     * @memberof exonum.examples.cryptocurrency_advanced.Transfer
                     * @instance
                     */
                    Transfer.prototype.seed = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
                    /**
                     * Creates a new Transfer instance using the specified properties.
                     * @function create
                     * @memberof exonum.examples.cryptocurrency_advanced.Transfer
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.ITransfer=} [properties] Properties to set
                     * @returns {exonum.examples.cryptocurrency_advanced.Transfer} Transfer instance
                     */
                    Transfer.create = function create(properties) {
                        return new Transfer(properties);
                    };
    
                    /**
                     * Encodes the specified Transfer message. Does not implicitly {@link exonum.examples.cryptocurrency_advanced.Transfer.verify|verify} messages.
                     * @function encode
                     * @memberof exonum.examples.cryptocurrency_advanced.Transfer
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.ITransfer} message Transfer message or plain object to encode
                     * @param {$protobuf.Writer} [writer] Writer to encode to
                     * @returns {$protobuf.Writer} Writer
                     */
                    Transfer.encode = function encode(message, writer) {
                        if (!writer)
                            writer = $Writer.create();
                        if (message.to != null && message.hasOwnProperty("to"))
                            $root.exonum.PublicKey.encode(message.to, writer.uint32(/* id 1, wireType 2 =*/10).fork()).ldelim();
                        if (message.amount != null && message.hasOwnProperty("amount"))
                            writer.uint32(/* id 2, wireType 0 =*/16).uint64(message.amount);
                        if (message.seed != null && message.hasOwnProperty("seed"))
                            writer.uint32(/* id 3, wireType 0 =*/24).uint64(message.seed);
                        return writer;
                    };
    
                    /**
                     * Encodes the specified Transfer message, length delimited. Does not implicitly {@link exonum.examples.cryptocurrency_advanced.Transfer.verify|verify} messages.
                     * @function encodeDelimited
                     * @memberof exonum.examples.cryptocurrency_advanced.Transfer
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.ITransfer} message Transfer message or plain object to encode
                     * @param {$protobuf.Writer} [writer] Writer to encode to
                     * @returns {$protobuf.Writer} Writer
                     */
                    Transfer.encodeDelimited = function encodeDelimited(message, writer) {
                        return this.encode(message, writer).ldelim();
                    };
    
                    /**
                     * Decodes a Transfer message from the specified reader or buffer.
                     * @function decode
                     * @memberof exonum.examples.cryptocurrency_advanced.Transfer
                     * @static
                     * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                     * @param {number} [length] Message length if known beforehand
                     * @returns {exonum.examples.cryptocurrency_advanced.Transfer} Transfer
                     * @throws {Error} If the payload is not a reader or valid buffer
                     * @throws {$protobuf.util.ProtocolError} If required fields are missing
                     */
                    Transfer.decode = function decode(reader, length) {
                        if (!(reader instanceof $Reader))
                            reader = $Reader.create(reader);
                        var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.examples.cryptocurrency_advanced.Transfer();
                        while (reader.pos < end) {
                            var tag = reader.uint32();
                            switch (tag >>> 3) {
                            case 1:
                                message.to = $root.exonum.PublicKey.decode(reader, reader.uint32());
                                break;
                            case 2:
                                message.amount = reader.uint64();
                                break;
                            case 3:
                                message.seed = reader.uint64();
                                break;
                            default:
                                reader.skipType(tag & 7);
                                break;
                            }
                        }
                        return message;
                    };
    
                    /**
                     * Decodes a Transfer message from the specified reader or buffer, length delimited.
                     * @function decodeDelimited
                     * @memberof exonum.examples.cryptocurrency_advanced.Transfer
                     * @static
                     * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                     * @returns {exonum.examples.cryptocurrency_advanced.Transfer} Transfer
                     * @throws {Error} If the payload is not a reader or valid buffer
                     * @throws {$protobuf.util.ProtocolError} If required fields are missing
                     */
                    Transfer.decodeDelimited = function decodeDelimited(reader) {
                        if (!(reader instanceof $Reader))
                            reader = new $Reader(reader);
                        return this.decode(reader, reader.uint32());
                    };
    
                    /**
                     * Verifies a Transfer message.
                     * @function verify
                     * @memberof exonum.examples.cryptocurrency_advanced.Transfer
                     * @static
                     * @param {Object.<string,*>} message Plain object to verify
                     * @returns {string|null} `null` if valid, otherwise the reason why it is not
                     */
                    Transfer.verify = function verify(message) {
                        if (typeof message !== "object" || message === null)
                            return "object expected";
                        if (message.to != null && message.hasOwnProperty("to")) {
                            var error = $root.exonum.PublicKey.verify(message.to);
                            if (error)
                                return "to." + error;
                        }
                        if (message.amount != null && message.hasOwnProperty("amount"))
                            if (!$util.isInteger(message.amount) && !(message.amount && $util.isInteger(message.amount.low) && $util.isInteger(message.amount.high)))
                                return "amount: integer|Long expected";
                        if (message.seed != null && message.hasOwnProperty("seed"))
                            if (!$util.isInteger(message.seed) && !(message.seed && $util.isInteger(message.seed.low) && $util.isInteger(message.seed.high)))
                                return "seed: integer|Long expected";
                        return null;
                    };
    
                    /**
                     * Creates a Transfer message from a plain object. Also converts values to their respective internal types.
                     * @function fromObject
                     * @memberof exonum.examples.cryptocurrency_advanced.Transfer
                     * @static
                     * @param {Object.<string,*>} object Plain object
                     * @returns {exonum.examples.cryptocurrency_advanced.Transfer} Transfer
                     */
                    Transfer.fromObject = function fromObject(object) {
                        if (object instanceof $root.exonum.examples.cryptocurrency_advanced.Transfer)
                            return object;
                        var message = new $root.exonum.examples.cryptocurrency_advanced.Transfer();
                        if (object.to != null) {
                            if (typeof object.to !== "object")
                                throw TypeError(".exonum.examples.cryptocurrency_advanced.Transfer.to: object expected");
                            message.to = $root.exonum.PublicKey.fromObject(object.to);
                        }
                        if (object.amount != null)
                            if ($util.Long)
                                (message.amount = $util.Long.fromValue(object.amount)).unsigned = true;
                            else if (typeof object.amount === "string")
                                message.amount = parseInt(object.amount, 10);
                            else if (typeof object.amount === "number")
                                message.amount = object.amount;
                            else if (typeof object.amount === "object")
                                message.amount = new $util.LongBits(object.amount.low >>> 0, object.amount.high >>> 0).toNumber(true);
                        if (object.seed != null)
                            if ($util.Long)
                                (message.seed = $util.Long.fromValue(object.seed)).unsigned = true;
                            else if (typeof object.seed === "string")
                                message.seed = parseInt(object.seed, 10);
                            else if (typeof object.seed === "number")
                                message.seed = object.seed;
                            else if (typeof object.seed === "object")
                                message.seed = new $util.LongBits(object.seed.low >>> 0, object.seed.high >>> 0).toNumber(true);
                        return message;
                    };
    
                    /**
                     * Creates a plain object from a Transfer message. Also converts values to other types if specified.
                     * @function toObject
                     * @memberof exonum.examples.cryptocurrency_advanced.Transfer
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.Transfer} message Transfer
                     * @param {$protobuf.IConversionOptions} [options] Conversion options
                     * @returns {Object.<string,*>} Plain object
                     */
                    Transfer.toObject = function toObject(message, options) {
                        if (!options)
                            options = {};
                        var object = {};
                        if (options.defaults) {
                            object.to = null;
                            if ($util.Long) {
                                var long = new $util.Long(0, 0, true);
                                object.amount = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                            } else
                                object.amount = options.longs === String ? "0" : 0;
                            if ($util.Long) {
                                var long = new $util.Long(0, 0, true);
                                object.seed = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                            } else
                                object.seed = options.longs === String ? "0" : 0;
                        }
                        if (message.to != null && message.hasOwnProperty("to"))
                            object.to = $root.exonum.PublicKey.toObject(message.to, options);
                        if (message.amount != null && message.hasOwnProperty("amount"))
                            if (typeof message.amount === "number")
                                object.amount = options.longs === String ? String(message.amount) : message.amount;
                            else
                                object.amount = options.longs === String ? $util.Long.prototype.toString.call(message.amount) : options.longs === Number ? new $util.LongBits(message.amount.low >>> 0, message.amount.high >>> 0).toNumber(true) : message.amount;
                        if (message.seed != null && message.hasOwnProperty("seed"))
                            if (typeof message.seed === "number")
                                object.seed = options.longs === String ? String(message.seed) : message.seed;
                            else
                                object.seed = options.longs === String ? $util.Long.prototype.toString.call(message.seed) : options.longs === Number ? new $util.LongBits(message.seed.low >>> 0, message.seed.high >>> 0).toNumber(true) : message.seed;
                        return object;
                    };
    
                    /**
                     * Converts this Transfer to JSON.
                     * @function toJSON
                     * @memberof exonum.examples.cryptocurrency_advanced.Transfer
                     * @instance
                     * @returns {Object.<string,*>} JSON object
                     */
                    Transfer.prototype.toJSON = function toJSON() {
                        return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                    };
    
                    return Transfer;
                })();
    
                cryptocurrency_advanced.Issue = (function() {
    
                    /**
                     * Properties of an Issue.
                     * @memberof exonum.examples.cryptocurrency_advanced
                     * @interface IIssue
                     * @property {number|Long|null} [amount] Issue amount
                     * @property {number|Long|null} [seed] Issue seed
                     */
    
                    /**
                     * Constructs a new Issue.
                     * @memberof exonum.examples.cryptocurrency_advanced
                     * @classdesc Represents an Issue.
                     * @implements IIssue
                     * @constructor
                     * @param {exonum.examples.cryptocurrency_advanced.IIssue=} [properties] Properties to set
                     */
                    function Issue(properties) {
                        if (properties)
                            for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                                if (properties[keys[i]] != null)
                                    this[keys[i]] = properties[keys[i]];
                    }
    
                    /**
                     * Issue amount.
                     * @member {number|Long} amount
                     * @memberof exonum.examples.cryptocurrency_advanced.Issue
                     * @instance
                     */
                    Issue.prototype.amount = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
                    /**
                     * Issue seed.
                     * @member {number|Long} seed
                     * @memberof exonum.examples.cryptocurrency_advanced.Issue
                     * @instance
                     */
                    Issue.prototype.seed = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
                    /**
                     * Creates a new Issue instance using the specified properties.
                     * @function create
                     * @memberof exonum.examples.cryptocurrency_advanced.Issue
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.IIssue=} [properties] Properties to set
                     * @returns {exonum.examples.cryptocurrency_advanced.Issue} Issue instance
                     */
                    Issue.create = function create(properties) {
                        return new Issue(properties);
                    };
    
                    /**
                     * Encodes the specified Issue message. Does not implicitly {@link exonum.examples.cryptocurrency_advanced.Issue.verify|verify} messages.
                     * @function encode
                     * @memberof exonum.examples.cryptocurrency_advanced.Issue
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.IIssue} message Issue message or plain object to encode
                     * @param {$protobuf.Writer} [writer] Writer to encode to
                     * @returns {$protobuf.Writer} Writer
                     */
                    Issue.encode = function encode(message, writer) {
                        if (!writer)
                            writer = $Writer.create();
                        if (message.amount != null && message.hasOwnProperty("amount"))
                            writer.uint32(/* id 1, wireType 0 =*/8).uint64(message.amount);
                        if (message.seed != null && message.hasOwnProperty("seed"))
                            writer.uint32(/* id 2, wireType 0 =*/16).uint64(message.seed);
                        return writer;
                    };
    
                    /**
                     * Encodes the specified Issue message, length delimited. Does not implicitly {@link exonum.examples.cryptocurrency_advanced.Issue.verify|verify} messages.
                     * @function encodeDelimited
                     * @memberof exonum.examples.cryptocurrency_advanced.Issue
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.IIssue} message Issue message or plain object to encode
                     * @param {$protobuf.Writer} [writer] Writer to encode to
                     * @returns {$protobuf.Writer} Writer
                     */
                    Issue.encodeDelimited = function encodeDelimited(message, writer) {
                        return this.encode(message, writer).ldelim();
                    };
    
                    /**
                     * Decodes an Issue message from the specified reader or buffer.
                     * @function decode
                     * @memberof exonum.examples.cryptocurrency_advanced.Issue
                     * @static
                     * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                     * @param {number} [length] Message length if known beforehand
                     * @returns {exonum.examples.cryptocurrency_advanced.Issue} Issue
                     * @throws {Error} If the payload is not a reader or valid buffer
                     * @throws {$protobuf.util.ProtocolError} If required fields are missing
                     */
                    Issue.decode = function decode(reader, length) {
                        if (!(reader instanceof $Reader))
                            reader = $Reader.create(reader);
                        var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.examples.cryptocurrency_advanced.Issue();
                        while (reader.pos < end) {
                            var tag = reader.uint32();
                            switch (tag >>> 3) {
                            case 1:
                                message.amount = reader.uint64();
                                break;
                            case 2:
                                message.seed = reader.uint64();
                                break;
                            default:
                                reader.skipType(tag & 7);
                                break;
                            }
                        }
                        return message;
                    };
    
                    /**
                     * Decodes an Issue message from the specified reader or buffer, length delimited.
                     * @function decodeDelimited
                     * @memberof exonum.examples.cryptocurrency_advanced.Issue
                     * @static
                     * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                     * @returns {exonum.examples.cryptocurrency_advanced.Issue} Issue
                     * @throws {Error} If the payload is not a reader or valid buffer
                     * @throws {$protobuf.util.ProtocolError} If required fields are missing
                     */
                    Issue.decodeDelimited = function decodeDelimited(reader) {
                        if (!(reader instanceof $Reader))
                            reader = new $Reader(reader);
                        return this.decode(reader, reader.uint32());
                    };
    
                    /**
                     * Verifies an Issue message.
                     * @function verify
                     * @memberof exonum.examples.cryptocurrency_advanced.Issue
                     * @static
                     * @param {Object.<string,*>} message Plain object to verify
                     * @returns {string|null} `null` if valid, otherwise the reason why it is not
                     */
                    Issue.verify = function verify(message) {
                        if (typeof message !== "object" || message === null)
                            return "object expected";
                        if (message.amount != null && message.hasOwnProperty("amount"))
                            if (!$util.isInteger(message.amount) && !(message.amount && $util.isInteger(message.amount.low) && $util.isInteger(message.amount.high)))
                                return "amount: integer|Long expected";
                        if (message.seed != null && message.hasOwnProperty("seed"))
                            if (!$util.isInteger(message.seed) && !(message.seed && $util.isInteger(message.seed.low) && $util.isInteger(message.seed.high)))
                                return "seed: integer|Long expected";
                        return null;
                    };
    
                    /**
                     * Creates an Issue message from a plain object. Also converts values to their respective internal types.
                     * @function fromObject
                     * @memberof exonum.examples.cryptocurrency_advanced.Issue
                     * @static
                     * @param {Object.<string,*>} object Plain object
                     * @returns {exonum.examples.cryptocurrency_advanced.Issue} Issue
                     */
                    Issue.fromObject = function fromObject(object) {
                        if (object instanceof $root.exonum.examples.cryptocurrency_advanced.Issue)
                            return object;
                        var message = new $root.exonum.examples.cryptocurrency_advanced.Issue();
                        if (object.amount != null)
                            if ($util.Long)
                                (message.amount = $util.Long.fromValue(object.amount)).unsigned = true;
                            else if (typeof object.amount === "string")
                                message.amount = parseInt(object.amount, 10);
                            else if (typeof object.amount === "number")
                                message.amount = object.amount;
                            else if (typeof object.amount === "object")
                                message.amount = new $util.LongBits(object.amount.low >>> 0, object.amount.high >>> 0).toNumber(true);
                        if (object.seed != null)
                            if ($util.Long)
                                (message.seed = $util.Long.fromValue(object.seed)).unsigned = true;
                            else if (typeof object.seed === "string")
                                message.seed = parseInt(object.seed, 10);
                            else if (typeof object.seed === "number")
                                message.seed = object.seed;
                            else if (typeof object.seed === "object")
                                message.seed = new $util.LongBits(object.seed.low >>> 0, object.seed.high >>> 0).toNumber(true);
                        return message;
                    };
    
                    /**
                     * Creates a plain object from an Issue message. Also converts values to other types if specified.
                     * @function toObject
                     * @memberof exonum.examples.cryptocurrency_advanced.Issue
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.Issue} message Issue
                     * @param {$protobuf.IConversionOptions} [options] Conversion options
                     * @returns {Object.<string,*>} Plain object
                     */
                    Issue.toObject = function toObject(message, options) {
                        if (!options)
                            options = {};
                        var object = {};
                        if (options.defaults) {
                            if ($util.Long) {
                                var long = new $util.Long(0, 0, true);
                                object.amount = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                            } else
                                object.amount = options.longs === String ? "0" : 0;
                            if ($util.Long) {
                                var long = new $util.Long(0, 0, true);
                                object.seed = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                            } else
                                object.seed = options.longs === String ? "0" : 0;
                        }
                        if (message.amount != null && message.hasOwnProperty("amount"))
                            if (typeof message.amount === "number")
                                object.amount = options.longs === String ? String(message.amount) : message.amount;
                            else
                                object.amount = options.longs === String ? $util.Long.prototype.toString.call(message.amount) : options.longs === Number ? new $util.LongBits(message.amount.low >>> 0, message.amount.high >>> 0).toNumber(true) : message.amount;
                        if (message.seed != null && message.hasOwnProperty("seed"))
                            if (typeof message.seed === "number")
                                object.seed = options.longs === String ? String(message.seed) : message.seed;
                            else
                                object.seed = options.longs === String ? $util.Long.prototype.toString.call(message.seed) : options.longs === Number ? new $util.LongBits(message.seed.low >>> 0, message.seed.high >>> 0).toNumber(true) : message.seed;
                        return object;
                    };
    
                    /**
                     * Converts this Issue to JSON.
                     * @function toJSON
                     * @memberof exonum.examples.cryptocurrency_advanced.Issue
                     * @instance
                     * @returns {Object.<string,*>} JSON object
                     */
                    Issue.prototype.toJSON = function toJSON() {
                        return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                    };
    
                    return Issue;
                })();
    
                cryptocurrency_advanced.CreateWallet = (function() {
    
                    /**
                     * Properties of a CreateWallet.
                     * @memberof exonum.examples.cryptocurrency_advanced
                     * @interface ICreateWallet
                     * @property {string|null} [name] CreateWallet name
                     */
    
                    /**
                     * Constructs a new CreateWallet.
                     * @memberof exonum.examples.cryptocurrency_advanced
                     * @classdesc Represents a CreateWallet.
                     * @implements ICreateWallet
                     * @constructor
                     * @param {exonum.examples.cryptocurrency_advanced.ICreateWallet=} [properties] Properties to set
                     */
                    function CreateWallet(properties) {
                        if (properties)
                            for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                                if (properties[keys[i]] != null)
                                    this[keys[i]] = properties[keys[i]];
                    }
    
                    /**
                     * CreateWallet name.
                     * @member {string} name
                     * @memberof exonum.examples.cryptocurrency_advanced.CreateWallet
                     * @instance
                     */
                    CreateWallet.prototype.name = "";
    
                    /**
                     * Creates a new CreateWallet instance using the specified properties.
                     * @function create
                     * @memberof exonum.examples.cryptocurrency_advanced.CreateWallet
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.ICreateWallet=} [properties] Properties to set
                     * @returns {exonum.examples.cryptocurrency_advanced.CreateWallet} CreateWallet instance
                     */
                    CreateWallet.create = function create(properties) {
                        return new CreateWallet(properties);
                    };
    
                    /**
                     * Encodes the specified CreateWallet message. Does not implicitly {@link exonum.examples.cryptocurrency_advanced.CreateWallet.verify|verify} messages.
                     * @function encode
                     * @memberof exonum.examples.cryptocurrency_advanced.CreateWallet
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.ICreateWallet} message CreateWallet message or plain object to encode
                     * @param {$protobuf.Writer} [writer] Writer to encode to
                     * @returns {$protobuf.Writer} Writer
                     */
                    CreateWallet.encode = function encode(message, writer) {
                        if (!writer)
                            writer = $Writer.create();
                        if (message.name != null && message.hasOwnProperty("name"))
                            writer.uint32(/* id 1, wireType 2 =*/10).string(message.name);
                        return writer;
                    };
    
                    /**
                     * Encodes the specified CreateWallet message, length delimited. Does not implicitly {@link exonum.examples.cryptocurrency_advanced.CreateWallet.verify|verify} messages.
                     * @function encodeDelimited
                     * @memberof exonum.examples.cryptocurrency_advanced.CreateWallet
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.ICreateWallet} message CreateWallet message or plain object to encode
                     * @param {$protobuf.Writer} [writer] Writer to encode to
                     * @returns {$protobuf.Writer} Writer
                     */
                    CreateWallet.encodeDelimited = function encodeDelimited(message, writer) {
                        return this.encode(message, writer).ldelim();
                    };
    
                    /**
                     * Decodes a CreateWallet message from the specified reader or buffer.
                     * @function decode
                     * @memberof exonum.examples.cryptocurrency_advanced.CreateWallet
                     * @static
                     * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                     * @param {number} [length] Message length if known beforehand
                     * @returns {exonum.examples.cryptocurrency_advanced.CreateWallet} CreateWallet
                     * @throws {Error} If the payload is not a reader or valid buffer
                     * @throws {$protobuf.util.ProtocolError} If required fields are missing
                     */
                    CreateWallet.decode = function decode(reader, length) {
                        if (!(reader instanceof $Reader))
                            reader = $Reader.create(reader);
                        var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.examples.cryptocurrency_advanced.CreateWallet();
                        while (reader.pos < end) {
                            var tag = reader.uint32();
                            switch (tag >>> 3) {
                            case 1:
                                message.name = reader.string();
                                break;
                            default:
                                reader.skipType(tag & 7);
                                break;
                            }
                        }
                        return message;
                    };
    
                    /**
                     * Decodes a CreateWallet message from the specified reader or buffer, length delimited.
                     * @function decodeDelimited
                     * @memberof exonum.examples.cryptocurrency_advanced.CreateWallet
                     * @static
                     * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                     * @returns {exonum.examples.cryptocurrency_advanced.CreateWallet} CreateWallet
                     * @throws {Error} If the payload is not a reader or valid buffer
                     * @throws {$protobuf.util.ProtocolError} If required fields are missing
                     */
                    CreateWallet.decodeDelimited = function decodeDelimited(reader) {
                        if (!(reader instanceof $Reader))
                            reader = new $Reader(reader);
                        return this.decode(reader, reader.uint32());
                    };
    
                    /**
                     * Verifies a CreateWallet message.
                     * @function verify
                     * @memberof exonum.examples.cryptocurrency_advanced.CreateWallet
                     * @static
                     * @param {Object.<string,*>} message Plain object to verify
                     * @returns {string|null} `null` if valid, otherwise the reason why it is not
                     */
                    CreateWallet.verify = function verify(message) {
                        if (typeof message !== "object" || message === null)
                            return "object expected";
                        if (message.name != null && message.hasOwnProperty("name"))
                            if (!$util.isString(message.name))
                                return "name: string expected";
                        return null;
                    };
    
                    /**
                     * Creates a CreateWallet message from a plain object. Also converts values to their respective internal types.
                     * @function fromObject
                     * @memberof exonum.examples.cryptocurrency_advanced.CreateWallet
                     * @static
                     * @param {Object.<string,*>} object Plain object
                     * @returns {exonum.examples.cryptocurrency_advanced.CreateWallet} CreateWallet
                     */
                    CreateWallet.fromObject = function fromObject(object) {
                        if (object instanceof $root.exonum.examples.cryptocurrency_advanced.CreateWallet)
                            return object;
                        var message = new $root.exonum.examples.cryptocurrency_advanced.CreateWallet();
                        if (object.name != null)
                            message.name = String(object.name);
                        return message;
                    };
    
                    /**
                     * Creates a plain object from a CreateWallet message. Also converts values to other types if specified.
                     * @function toObject
                     * @memberof exonum.examples.cryptocurrency_advanced.CreateWallet
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.CreateWallet} message CreateWallet
                     * @param {$protobuf.IConversionOptions} [options] Conversion options
                     * @returns {Object.<string,*>} Plain object
                     */
                    CreateWallet.toObject = function toObject(message, options) {
                        if (!options)
                            options = {};
                        var object = {};
                        if (options.defaults)
                            object.name = "";
                        if (message.name != null && message.hasOwnProperty("name"))
                            object.name = message.name;
                        return object;
                    };
    
                    /**
                     * Converts this CreateWallet to JSON.
                     * @function toJSON
                     * @memberof exonum.examples.cryptocurrency_advanced.CreateWallet
                     * @instance
                     * @returns {Object.<string,*>} JSON object
                     */
                    CreateWallet.prototype.toJSON = function toJSON() {
                        return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                    };
    
                    return CreateWallet;
                })();
    
                cryptocurrency_advanced.Wallet = (function() {
    
                    /**
                     * Properties of a Wallet.
                     * @memberof exonum.examples.cryptocurrency_advanced
                     * @interface IWallet
                     * @property {exonum.IPublicKey|null} [pubKey] Wallet pubKey
                     * @property {string|null} [name] Wallet name
                     * @property {number|Long|null} [balance] Wallet balance
                     * @property {number|Long|null} [historyLen] Wallet historyLen
                     * @property {exonum.IHash|null} [historyHash] Wallet historyHash
                     */
    
                    /**
                     * Constructs a new Wallet.
                     * @memberof exonum.examples.cryptocurrency_advanced
                     * @classdesc Represents a Wallet.
                     * @implements IWallet
                     * @constructor
                     * @param {exonum.examples.cryptocurrency_advanced.IWallet=} [properties] Properties to set
                     */
                    function Wallet(properties) {
                        if (properties)
                            for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                                if (properties[keys[i]] != null)
                                    this[keys[i]] = properties[keys[i]];
                    }
    
                    /**
                     * Wallet pubKey.
                     * @member {exonum.IPublicKey|null|undefined} pubKey
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @instance
                     */
                    Wallet.prototype.pubKey = null;
    
                    /**
                     * Wallet name.
                     * @member {string} name
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @instance
                     */
                    Wallet.prototype.name = "";
    
                    /**
                     * Wallet balance.
                     * @member {number|Long} balance
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @instance
                     */
                    Wallet.prototype.balance = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
                    /**
                     * Wallet historyLen.
                     * @member {number|Long} historyLen
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @instance
                     */
                    Wallet.prototype.historyLen = $util.Long ? $util.Long.fromBits(0,0,true) : 0;
    
                    /**
                     * Wallet historyHash.
                     * @member {exonum.IHash|null|undefined} historyHash
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @instance
                     */
                    Wallet.prototype.historyHash = null;
    
                    /**
                     * Creates a new Wallet instance using the specified properties.
                     * @function create
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.IWallet=} [properties] Properties to set
                     * @returns {exonum.examples.cryptocurrency_advanced.Wallet} Wallet instance
                     */
                    Wallet.create = function create(properties) {
                        return new Wallet(properties);
                    };
    
                    /**
                     * Encodes the specified Wallet message. Does not implicitly {@link exonum.examples.cryptocurrency_advanced.Wallet.verify|verify} messages.
                     * @function encode
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.IWallet} message Wallet message or plain object to encode
                     * @param {$protobuf.Writer} [writer] Writer to encode to
                     * @returns {$protobuf.Writer} Writer
                     */
                    Wallet.encode = function encode(message, writer) {
                        if (!writer)
                            writer = $Writer.create();
                        if (message.pubKey != null && message.hasOwnProperty("pubKey"))
                            $root.exonum.PublicKey.encode(message.pubKey, writer.uint32(/* id 1, wireType 2 =*/10).fork()).ldelim();
                        if (message.name != null && message.hasOwnProperty("name"))
                            writer.uint32(/* id 2, wireType 2 =*/18).string(message.name);
                        if (message.balance != null && message.hasOwnProperty("balance"))
                            writer.uint32(/* id 3, wireType 0 =*/24).uint64(message.balance);
                        if (message.historyLen != null && message.hasOwnProperty("historyLen"))
                            writer.uint32(/* id 4, wireType 0 =*/32).uint64(message.historyLen);
                        if (message.historyHash != null && message.hasOwnProperty("historyHash"))
                            $root.exonum.Hash.encode(message.historyHash, writer.uint32(/* id 5, wireType 2 =*/42).fork()).ldelim();
                        return writer;
                    };
    
                    /**
                     * Encodes the specified Wallet message, length delimited. Does not implicitly {@link exonum.examples.cryptocurrency_advanced.Wallet.verify|verify} messages.
                     * @function encodeDelimited
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.IWallet} message Wallet message or plain object to encode
                     * @param {$protobuf.Writer} [writer] Writer to encode to
                     * @returns {$protobuf.Writer} Writer
                     */
                    Wallet.encodeDelimited = function encodeDelimited(message, writer) {
                        return this.encode(message, writer).ldelim();
                    };
    
                    /**
                     * Decodes a Wallet message from the specified reader or buffer.
                     * @function decode
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @static
                     * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                     * @param {number} [length] Message length if known beforehand
                     * @returns {exonum.examples.cryptocurrency_advanced.Wallet} Wallet
                     * @throws {Error} If the payload is not a reader or valid buffer
                     * @throws {$protobuf.util.ProtocolError} If required fields are missing
                     */
                    Wallet.decode = function decode(reader, length) {
                        if (!(reader instanceof $Reader))
                            reader = $Reader.create(reader);
                        var end = length === undefined ? reader.len : reader.pos + length, message = new $root.exonum.examples.cryptocurrency_advanced.Wallet();
                        while (reader.pos < end) {
                            var tag = reader.uint32();
                            switch (tag >>> 3) {
                            case 1:
                                message.pubKey = $root.exonum.PublicKey.decode(reader, reader.uint32());
                                break;
                            case 2:
                                message.name = reader.string();
                                break;
                            case 3:
                                message.balance = reader.uint64();
                                break;
                            case 4:
                                message.historyLen = reader.uint64();
                                break;
                            case 5:
                                message.historyHash = $root.exonum.Hash.decode(reader, reader.uint32());
                                break;
                            default:
                                reader.skipType(tag & 7);
                                break;
                            }
                        }
                        return message;
                    };
    
                    /**
                     * Decodes a Wallet message from the specified reader or buffer, length delimited.
                     * @function decodeDelimited
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @static
                     * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                     * @returns {exonum.examples.cryptocurrency_advanced.Wallet} Wallet
                     * @throws {Error} If the payload is not a reader or valid buffer
                     * @throws {$protobuf.util.ProtocolError} If required fields are missing
                     */
                    Wallet.decodeDelimited = function decodeDelimited(reader) {
                        if (!(reader instanceof $Reader))
                            reader = new $Reader(reader);
                        return this.decode(reader, reader.uint32());
                    };
    
                    /**
                     * Verifies a Wallet message.
                     * @function verify
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @static
                     * @param {Object.<string,*>} message Plain object to verify
                     * @returns {string|null} `null` if valid, otherwise the reason why it is not
                     */
                    Wallet.verify = function verify(message) {
                        if (typeof message !== "object" || message === null)
                            return "object expected";
                        if (message.pubKey != null && message.hasOwnProperty("pubKey")) {
                            var error = $root.exonum.PublicKey.verify(message.pubKey);
                            if (error)
                                return "pubKey." + error;
                        }
                        if (message.name != null && message.hasOwnProperty("name"))
                            if (!$util.isString(message.name))
                                return "name: string expected";
                        if (message.balance != null && message.hasOwnProperty("balance"))
                            if (!$util.isInteger(message.balance) && !(message.balance && $util.isInteger(message.balance.low) && $util.isInteger(message.balance.high)))
                                return "balance: integer|Long expected";
                        if (message.historyLen != null && message.hasOwnProperty("historyLen"))
                            if (!$util.isInteger(message.historyLen) && !(message.historyLen && $util.isInteger(message.historyLen.low) && $util.isInteger(message.historyLen.high)))
                                return "historyLen: integer|Long expected";
                        if (message.historyHash != null && message.hasOwnProperty("historyHash")) {
                            var error = $root.exonum.Hash.verify(message.historyHash);
                            if (error)
                                return "historyHash." + error;
                        }
                        return null;
                    };
    
                    /**
                     * Creates a Wallet message from a plain object. Also converts values to their respective internal types.
                     * @function fromObject
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @static
                     * @param {Object.<string,*>} object Plain object
                     * @returns {exonum.examples.cryptocurrency_advanced.Wallet} Wallet
                     */
                    Wallet.fromObject = function fromObject(object) {
                        if (object instanceof $root.exonum.examples.cryptocurrency_advanced.Wallet)
                            return object;
                        var message = new $root.exonum.examples.cryptocurrency_advanced.Wallet();
                        if (object.pubKey != null) {
                            if (typeof object.pubKey !== "object")
                                throw TypeError(".exonum.examples.cryptocurrency_advanced.Wallet.pubKey: object expected");
                            message.pubKey = $root.exonum.PublicKey.fromObject(object.pubKey);
                        }
                        if (object.name != null)
                            message.name = String(object.name);
                        if (object.balance != null)
                            if ($util.Long)
                                (message.balance = $util.Long.fromValue(object.balance)).unsigned = true;
                            else if (typeof object.balance === "string")
                                message.balance = parseInt(object.balance, 10);
                            else if (typeof object.balance === "number")
                                message.balance = object.balance;
                            else if (typeof object.balance === "object")
                                message.balance = new $util.LongBits(object.balance.low >>> 0, object.balance.high >>> 0).toNumber(true);
                        if (object.historyLen != null)
                            if ($util.Long)
                                (message.historyLen = $util.Long.fromValue(object.historyLen)).unsigned = true;
                            else if (typeof object.historyLen === "string")
                                message.historyLen = parseInt(object.historyLen, 10);
                            else if (typeof object.historyLen === "number")
                                message.historyLen = object.historyLen;
                            else if (typeof object.historyLen === "object")
                                message.historyLen = new $util.LongBits(object.historyLen.low >>> 0, object.historyLen.high >>> 0).toNumber(true);
                        if (object.historyHash != null) {
                            if (typeof object.historyHash !== "object")
                                throw TypeError(".exonum.examples.cryptocurrency_advanced.Wallet.historyHash: object expected");
                            message.historyHash = $root.exonum.Hash.fromObject(object.historyHash);
                        }
                        return message;
                    };
    
                    /**
                     * Creates a plain object from a Wallet message. Also converts values to other types if specified.
                     * @function toObject
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @static
                     * @param {exonum.examples.cryptocurrency_advanced.Wallet} message Wallet
                     * @param {$protobuf.IConversionOptions} [options] Conversion options
                     * @returns {Object.<string,*>} Plain object
                     */
                    Wallet.toObject = function toObject(message, options) {
                        if (!options)
                            options = {};
                        var object = {};
                        if (options.defaults) {
                            object.pubKey = null;
                            object.name = "";
                            if ($util.Long) {
                                var long = new $util.Long(0, 0, true);
                                object.balance = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                            } else
                                object.balance = options.longs === String ? "0" : 0;
                            if ($util.Long) {
                                var long = new $util.Long(0, 0, true);
                                object.historyLen = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                            } else
                                object.historyLen = options.longs === String ? "0" : 0;
                            object.historyHash = null;
                        }
                        if (message.pubKey != null && message.hasOwnProperty("pubKey"))
                            object.pubKey = $root.exonum.PublicKey.toObject(message.pubKey, options);
                        if (message.name != null && message.hasOwnProperty("name"))
                            object.name = message.name;
                        if (message.balance != null && message.hasOwnProperty("balance"))
                            if (typeof message.balance === "number")
                                object.balance = options.longs === String ? String(message.balance) : message.balance;
                            else
                                object.balance = options.longs === String ? $util.Long.prototype.toString.call(message.balance) : options.longs === Number ? new $util.LongBits(message.balance.low >>> 0, message.balance.high >>> 0).toNumber(true) : message.balance;
                        if (message.historyLen != null && message.hasOwnProperty("historyLen"))
                            if (typeof message.historyLen === "number")
                                object.historyLen = options.longs === String ? String(message.historyLen) : message.historyLen;
                            else
                                object.historyLen = options.longs === String ? $util.Long.prototype.toString.call(message.historyLen) : options.longs === Number ? new $util.LongBits(message.historyLen.low >>> 0, message.historyLen.high >>> 0).toNumber(true) : message.historyLen;
                        if (message.historyHash != null && message.hasOwnProperty("historyHash"))
                            object.historyHash = $root.exonum.Hash.toObject(message.historyHash, options);
                        return object;
                    };
    
                    /**
                     * Converts this Wallet to JSON.
                     * @function toJSON
                     * @memberof exonum.examples.cryptocurrency_advanced.Wallet
                     * @instance
                     * @returns {Object.<string,*>} JSON object
                     */
                    Wallet.prototype.toJSON = function toJSON() {
                        return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                    };
    
                    return Wallet;
                })();
    
                return cryptocurrency_advanced;
            })();
    
            return examples;
        })();
    
        return exonum;
    })();
    
    $root.google = (function() {
    
        /**
         * Namespace google.
         * @exports google
         * @namespace
         */
        var google = {};
    
        google.protobuf = (function() {
    
            /**
             * Namespace protobuf.
             * @memberof google
             * @namespace
             */
            var protobuf = {};
    
            protobuf.Timestamp = (function() {
    
                /**
                 * Properties of a Timestamp.
                 * @memberof google.protobuf
                 * @interface ITimestamp
                 * @property {number|Long|null} [seconds] Timestamp seconds
                 * @property {number|null} [nanos] Timestamp nanos
                 */
    
                /**
                 * Constructs a new Timestamp.
                 * @memberof google.protobuf
                 * @classdesc Represents a Timestamp.
                 * @implements ITimestamp
                 * @constructor
                 * @param {google.protobuf.ITimestamp=} [properties] Properties to set
                 */
                function Timestamp(properties) {
                    if (properties)
                        for (var keys = Object.keys(properties), i = 0; i < keys.length; ++i)
                            if (properties[keys[i]] != null)
                                this[keys[i]] = properties[keys[i]];
                }
    
                /**
                 * Timestamp seconds.
                 * @member {number|Long} seconds
                 * @memberof google.protobuf.Timestamp
                 * @instance
                 */
                Timestamp.prototype.seconds = $util.Long ? $util.Long.fromBits(0,0,false) : 0;
    
                /**
                 * Timestamp nanos.
                 * @member {number} nanos
                 * @memberof google.protobuf.Timestamp
                 * @instance
                 */
                Timestamp.prototype.nanos = 0;
    
                /**
                 * Creates a new Timestamp instance using the specified properties.
                 * @function create
                 * @memberof google.protobuf.Timestamp
                 * @static
                 * @param {google.protobuf.ITimestamp=} [properties] Properties to set
                 * @returns {google.protobuf.Timestamp} Timestamp instance
                 */
                Timestamp.create = function create(properties) {
                    return new Timestamp(properties);
                };
    
                /**
                 * Encodes the specified Timestamp message. Does not implicitly {@link google.protobuf.Timestamp.verify|verify} messages.
                 * @function encode
                 * @memberof google.protobuf.Timestamp
                 * @static
                 * @param {google.protobuf.ITimestamp} message Timestamp message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                Timestamp.encode = function encode(message, writer) {
                    if (!writer)
                        writer = $Writer.create();
                    if (message.seconds != null && message.hasOwnProperty("seconds"))
                        writer.uint32(/* id 1, wireType 0 =*/8).int64(message.seconds);
                    if (message.nanos != null && message.hasOwnProperty("nanos"))
                        writer.uint32(/* id 2, wireType 0 =*/16).int32(message.nanos);
                    return writer;
                };
    
                /**
                 * Encodes the specified Timestamp message, length delimited. Does not implicitly {@link google.protobuf.Timestamp.verify|verify} messages.
                 * @function encodeDelimited
                 * @memberof google.protobuf.Timestamp
                 * @static
                 * @param {google.protobuf.ITimestamp} message Timestamp message or plain object to encode
                 * @param {$protobuf.Writer} [writer] Writer to encode to
                 * @returns {$protobuf.Writer} Writer
                 */
                Timestamp.encodeDelimited = function encodeDelimited(message, writer) {
                    return this.encode(message, writer).ldelim();
                };
    
                /**
                 * Decodes a Timestamp message from the specified reader or buffer.
                 * @function decode
                 * @memberof google.protobuf.Timestamp
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @param {number} [length] Message length if known beforehand
                 * @returns {google.protobuf.Timestamp} Timestamp
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                Timestamp.decode = function decode(reader, length) {
                    if (!(reader instanceof $Reader))
                        reader = $Reader.create(reader);
                    var end = length === undefined ? reader.len : reader.pos + length, message = new $root.google.protobuf.Timestamp();
                    while (reader.pos < end) {
                        var tag = reader.uint32();
                        switch (tag >>> 3) {
                        case 1:
                            message.seconds = reader.int64();
                            break;
                        case 2:
                            message.nanos = reader.int32();
                            break;
                        default:
                            reader.skipType(tag & 7);
                            break;
                        }
                    }
                    return message;
                };
    
                /**
                 * Decodes a Timestamp message from the specified reader or buffer, length delimited.
                 * @function decodeDelimited
                 * @memberof google.protobuf.Timestamp
                 * @static
                 * @param {$protobuf.Reader|Uint8Array} reader Reader or buffer to decode from
                 * @returns {google.protobuf.Timestamp} Timestamp
                 * @throws {Error} If the payload is not a reader or valid buffer
                 * @throws {$protobuf.util.ProtocolError} If required fields are missing
                 */
                Timestamp.decodeDelimited = function decodeDelimited(reader) {
                    if (!(reader instanceof $Reader))
                        reader = new $Reader(reader);
                    return this.decode(reader, reader.uint32());
                };
    
                /**
                 * Verifies a Timestamp message.
                 * @function verify
                 * @memberof google.protobuf.Timestamp
                 * @static
                 * @param {Object.<string,*>} message Plain object to verify
                 * @returns {string|null} `null` if valid, otherwise the reason why it is not
                 */
                Timestamp.verify = function verify(message) {
                    if (typeof message !== "object" || message === null)
                        return "object expected";
                    if (message.seconds != null && message.hasOwnProperty("seconds"))
                        if (!$util.isInteger(message.seconds) && !(message.seconds && $util.isInteger(message.seconds.low) && $util.isInteger(message.seconds.high)))
                            return "seconds: integer|Long expected";
                    if (message.nanos != null && message.hasOwnProperty("nanos"))
                        if (!$util.isInteger(message.nanos))
                            return "nanos: integer expected";
                    return null;
                };
    
                /**
                 * Creates a Timestamp message from a plain object. Also converts values to their respective internal types.
                 * @function fromObject
                 * @memberof google.protobuf.Timestamp
                 * @static
                 * @param {Object.<string,*>} object Plain object
                 * @returns {google.protobuf.Timestamp} Timestamp
                 */
                Timestamp.fromObject = function fromObject(object) {
                    if (object instanceof $root.google.protobuf.Timestamp)
                        return object;
                    var message = new $root.google.protobuf.Timestamp();
                    if (object.seconds != null)
                        if ($util.Long)
                            (message.seconds = $util.Long.fromValue(object.seconds)).unsigned = false;
                        else if (typeof object.seconds === "string")
                            message.seconds = parseInt(object.seconds, 10);
                        else if (typeof object.seconds === "number")
                            message.seconds = object.seconds;
                        else if (typeof object.seconds === "object")
                            message.seconds = new $util.LongBits(object.seconds.low >>> 0, object.seconds.high >>> 0).toNumber();
                    if (object.nanos != null)
                        message.nanos = object.nanos | 0;
                    return message;
                };
    
                /**
                 * Creates a plain object from a Timestamp message. Also converts values to other types if specified.
                 * @function toObject
                 * @memberof google.protobuf.Timestamp
                 * @static
                 * @param {google.protobuf.Timestamp} message Timestamp
                 * @param {$protobuf.IConversionOptions} [options] Conversion options
                 * @returns {Object.<string,*>} Plain object
                 */
                Timestamp.toObject = function toObject(message, options) {
                    if (!options)
                        options = {};
                    var object = {};
                    if (options.defaults) {
                        if ($util.Long) {
                            var long = new $util.Long(0, 0, false);
                            object.seconds = options.longs === String ? long.toString() : options.longs === Number ? long.toNumber() : long;
                        } else
                            object.seconds = options.longs === String ? "0" : 0;
                        object.nanos = 0;
                    }
                    if (message.seconds != null && message.hasOwnProperty("seconds"))
                        if (typeof message.seconds === "number")
                            object.seconds = options.longs === String ? String(message.seconds) : message.seconds;
                        else
                            object.seconds = options.longs === String ? $util.Long.prototype.toString.call(message.seconds) : options.longs === Number ? new $util.LongBits(message.seconds.low >>> 0, message.seconds.high >>> 0).toNumber() : message.seconds;
                    if (message.nanos != null && message.hasOwnProperty("nanos"))
                        object.nanos = message.nanos;
                    return object;
                };
    
                /**
                 * Converts this Timestamp to JSON.
                 * @function toJSON
                 * @memberof google.protobuf.Timestamp
                 * @instance
                 * @returns {Object.<string,*>} JSON object
                 */
                Timestamp.prototype.toJSON = function toJSON() {
                    return this.constructor.toObject(this, $protobuf.util.toJSONOptions);
                };
    
                return Timestamp;
            })();
    
            return protobuf;
        })();
    
        return google;
    })();

    return $root;
});
