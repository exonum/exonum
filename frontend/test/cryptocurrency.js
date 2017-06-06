var NETWORK_ID = 0;
var PROTOCOL_VERSION = 0;
var SERVICE_ID = 128;
var VALIDATORS = readJSON('test_data/validators2.json');
var CONFIGURATION = {
    network_id: NETWORK_ID,
    protocol_version: PROTOCOL_VERSION,
    service_id: SERVICE_ID,
    validators: VALIDATORS
};
var service = new CryptocurrencyService(CONFIGURATION);

describe('Configure service', function() {

    it('should correctly initialize if valid configuration is passed', function() {
        expect(service.configuration).to.deep.equal(CONFIGURATION);
    });

});

describe('Verify API requests', function() {

    var transactionUrl = 'api/services/cryptocurrency/v1/wallets/transaction';
    var server;
    var serverTimeout = 100;

    beforeEach(function() {
        server = sinon.fakeServer.create();

        sinon.spy(jQuery, 'ajax');
    });

    afterEach(function() {
        jQuery.ajax.restore();

        server.restore();
    });

    describe('Register', function() {

        var login = 'a';
        var password = 'a';

        it('should sign and submit transaction', function() {
            var response = {
                tx_hash: '71f134bb69f5e371ce17ec4ca46c891fae96ae1feac2e49c2e82ba046b277951'
            };

            server.respondWith('POST', transactionUrl, [200, {'Content-Type': 'application/json'}, JSON.stringify(response)]);

            service.createWallet(login, password);

            setTimeout(function() {
                server.respond();

                var spyCall = $.ajax.getCall(0);

                expect(JSON.parse(spyCall.args[0].data)).to.deep.equal({
                    "body": {
                        "login": "a",
                        "pub_key": "87f7bd7fe1394c57526b06565fdc5107c76a6cf703f6038b80bb10daebd57d62",
                        "key_box": "73637279707400000000080000000001eae91bc781165d6c35aa5ac0943c08b9e233c27faadf57f3e394af5616bf952421041cbb5254745027aa35dcabe5d52784d34a9571a23ba8e32ea93fd0cf1c7df467e4bbe9d0963fde44570ee7e8bdef177e3086795c9c36e3b5a826fb7f8fdf773a66996c7f2fe8b43ae1be4e13785d"
                    },
                    "network_id": 0,
                    "protocol_version": 0,
                    "service_id": 128,
                    "message_id": 130,
                    "signature": "a7d3897ff209b621257b6f726215bfb5a231832f4a86c5adf970dc01cf2deafb2e203cb73c04405df25f89d77483e4afe0226403b6cd6f306bf7acf9e42b260e"
                });

                expect(JSON.parse(spyCall.returnValue.responseText)).to.deep.equal(response);
            }, serverTimeout);
        });

        it('should throw error on server error', function() {
            server.respondWith('POST', transactionUrl, [404, {'Content-Type': 'application/json'}, '']);

            service.createWallet(login, password);

            setTimeout(function() {
                expect(function(){
                    server.respond();
                }).to.throw();
            }, serverTimeout);
        });

        it('should throw error on wrong format of server response', function() {
            server.respondWith('POST', transactionUrl, [200, {'Content-Type': 'application/json'}, '']);

            service.createWallet(login, password);

            setTimeout(function() {
                expect(function(){
                    server.respond();
                }).to.throw();
            }, serverTimeout);
        });

    });

    describe('Login', function() {

        var login = 'a';
        var password = 'a';
        var url = 'api/services/cryptocurrency/v1/wallets/find/';

        it('should sign and submit transaction', function() {
            var response = {
                "key_box": "73637279707400000000080000000001726ee990c698946b540c5d5a77efc887fc4baff92d889d63a490d703118ac8cf2ff3f18fb5ca2d9b18ae29ee3af06c535116f03c4a36ab1c648ddc585630291022f7fca68c01f7c9bf0e1d3a082a2c3a2769da06c9fa988ecbc60e213e296bc16914d2b97443a542bae1630b60d8e434",
                "pub_key": "2fc4c29755938792d36382adbe2850026e266eb256b125c756e784eaec29174f"
            };

            server.respondWith('GET', url + login, [200, {'Content-Type': 'application/json'}, JSON.stringify(response)]);

            service.login(login, password, function() {});

            setTimeout(function() {
                server.respond();

                var spyCall = $.ajax.getCall(0);

                expect(JSON.parse(spyCall.returnValue.responseText)).to.deep.equal(response);
            }, serverTimeout);
        });

        it('should throw error on server error', function() {
            server.respondWith('GET', url + login, [404, {'Content-Type': 'application/json'}, '']);

            service.login(login, password, function() {});

            setTimeout(function() {
                expect(function(){
                    server.respond();
                }).to.throw();
            }, serverTimeout);
        });

        it('should throw error on wrong format of server response', function() {
            server.respondWith('GET', url + login, [200, {'Content-Type': 'application/json'}, '']);

            service.login(login, password, function() {});

            setTimeout(function() {
                expect(function(){
                    server.respond();
                }).to.throw();
            }, serverTimeout);
        });

    });

    describe('Add Funds', function() {

        var publicKey = '03e657ae71e51be60a45b4bd20bcf79ff52f0c037ae6da0540a0e0066132b472';
        var secretKey = '2ef1f2e2799c93b50d2a7ba207a4efebebf6fe5735339dc782e06b1c30b72abf03e657ae71e51be60a45b4bd20bcf79ff52f0c037ae6da0540a0e0066132b472';
        var amount = '100';
        var response = {
            tx_hash: '7134f9b227f8a7ec553ea70e8422fac439b20066f9f23b88e780614fe4c70c26'
        };

        it('should sign and submit transaction', function() {
            server.respondWith('POST', transactionUrl, [200, {'Content-Type': 'application/json'}, JSON.stringify(response)]);

            service.addFunds(amount, publicKey, secretKey);

            server.respond();

            var spyCall = $.ajax.getCall(0);

            var requestData = JSON.parse(spyCall.args[0].data);

            expect(requestData.body.wallet).to.equal(publicKey);

            expect(requestData.body.amount).to.equal(amount);

            expect(JSON.parse(spyCall.returnValue.responseText)).to.deep.equal(response);
        });

        it('should throw error on server error', function() {
            server.respondWith('POST', transactionUrl, [404, {'Content-Type': 'application/json'}, '']);

            service.addFunds(amount, publicKey, secretKey);

            expect(function(){
                server.respond();
            }).to.throw();
        });

        it('should throw error on wrong format of server response', function() {
            server.respondWith('POST', transactionUrl, [200, {'Content-Type': 'application/json'}, '']);

            service.addFunds(amount, publicKey, secretKey);

            expect(function(){
                server.respond();
            }).to.throw();
        });

    });

    describe('Transfer Funds', function() {

        var publicKey = '03e657ae71e51be60a45b4bd20bcf79ff52f0c037ae6da0540a0e0066132b472';
        var secretKey = '2ef1f2e2799c93b50d2a7ba207a4efebebf6fe5735339dc782e06b1c30b72abf03e657ae71e51be60a45b4bd20bcf79ff52f0c037ae6da0540a0e0066132b472';
        var receiver = 'd1e877472a4585d515b13f52ae7bfded1ccea511816d7772cb17e1ab20830819';
        var amount = '100';
        var response = {
            tx_hash: '311d597446f1a19d3aac63e23c61a85d3c4cb0855f1085ed1c531a32cda47344'
        };

        it('should sign and submit transaction', function() {
            server.respondWith('POST', transactionUrl, [200, {'Content-Type': 'application/json'}, JSON.stringify(response)]);

            service.transfer(amount, publicKey, receiver, secretKey);

            server.respond();

            var spyCall = $.ajax.getCall(0);

            var requestData = JSON.parse(spyCall.args[0].data);

            expect(requestData.body.from).to.equal(publicKey);

            expect(requestData.body.to).to.equal(receiver);

            expect(requestData.body.amount).to.equal(amount);

            expect(JSON.parse(spyCall.returnValue.responseText)).to.deep.equal(response);
        });

        it('should throw error on server error', function() {
            server.respondWith('POST', transactionUrl, [404, {'Content-Type': 'application/json'}, '']);

            service.transfer(amount, publicKey, receiver, secretKey);

            expect(function(){
                server.respond();
            }).to.throw();
        });

        it('should throw error on wrong format of server response', function() {
            server.respondWith('POST', transactionUrl, [200, {'Content-Type': 'application/json'}, '']);

            service.transfer(amount, publicKey, receiver, secretKey);

            expect(function(){
                server.respond();
            }).to.throw();
        });

    });

});

describe('Wallet query validation', function() {

    var server;
    var url = 'api/services/cryptocurrency/v1/wallets/info?pubkey=';
    var publicKey = '2fc4c29755938792d36382adbe2850026e266eb256b125c756e784eaec29174f';
    var walletNew = readJSON('test_data/wallet_new.json');
    var walletOnAddFunds = readJSON('test_data/wallet_on_add_funds.json');
    var walletOnTransfer = readJSON('test_data/wallet_on_transfer.json');

    beforeEach(function() {
        server = sinon.fakeServer.create();
    });

    afterEach(function() {
        server.restore();
    });

    it('should return expected parameters on valid new wallet', function(done) {
        server.respondWith('GET', url + publicKey, [200, {'Content-Type': 'application/json'}, JSON.stringify(walletNew)]);

        service.getWallet(publicKey, function(error, block, wallet, transactions) {
            expect(block).to.deep.equal({
                height: '63636',
                prev_hash: '055f18a41e82d106c830ed8140b09e885daaddab8b429364369a0fb34be2586f',
                proposer_id: 1,
                schema_version: 0,
                state_hash: 'cfa66326b4865b1b396e916909f9af7fa22d231ced57a8fc575c611adf15a5bd',
                tx_count: 0,
                tx_hash: '0000000000000000000000000000000000000000000000000000000000000000',
                time: '1512914676062495000'
            });

            expect(wallet).to.deep.equal({
                "balance": "0",
                "history_hash": "1f4482e6ff3299dc2dda01f7635513c082c53e167535b690c2724090a65a4c19",
                "history_len": "1",
                "login": "abcd",
                "pub_key": "2fc4c29755938792d36382adbe2850026e266eb256b125c756e784eaec29174f"
            });

            expect(transactions.length).to.equal(1);

            done();
        });

        server.respond();
    });

    it('should return expected parameters on valid wallet after add funds', function(done) {
        server.respondWith('GET', url + publicKey, [200, {'Content-Type': 'application/json'}, JSON.stringify(walletOnAddFunds)]);

        service.getWallet(publicKey, function(error, block, wallet, transactions) {
            expect(block).to.deep.equal({
                height: '66060',
                prev_hash: '217d773050a3663e5c7e8fe83776a057a5e5c4782d420cb0536ba1684785be34',
                proposer_id: 1,
                schema_version: 0,
                state_hash: '6349ee85951d89c91ced99e02df728a2d76a31641a25c671a7bb9306e1322f6b',
                tx_count: 0,
                tx_hash: '0000000000000000000000000000000000000000000000000000000000000000',
                time: '1512915918506001000'
            });

            expect(wallet).to.deep.equal({
                "balance": "50",
                "history_hash": "e755cacedd370092c6da89bf4ccd3095486ea94214ccf75e6deef9bcb1616a04",
                "history_len": "2",
                "login": "abcd",
                "pub_key": "2fc4c29755938792d36382adbe2850026e266eb256b125c756e784eaec29174f"
            });

            expect(transactions.length).to.equal(2);

            done();
        });

        server.respond();
    });

    it('should return expected parameters on valid wallet after transfer funds', function(done) {
        server.respondWith('GET', url + publicKey, [200, {'Content-Type': 'application/json'}, JSON.stringify(walletOnTransfer)]);

        service.getWallet(publicKey, function(error, block, wallet, transactions) {
            expect(block).to.deep.equal({
                height: '66399',
                prev_hash: 'd71582601dd20fbfe7175890fa2816a97f76c0fbd6397edb5c51a8573886a4df',
                proposer_id: 4,
                schema_version: 0,
                state_hash: '2cf4f5fec47b570f33b6c9b40e3fd271b1cf3d8ff5c24809df1cbaa41c0e8473',
                tx_count: 0,
                tx_hash: '0000000000000000000000000000000000000000000000000000000000000000',
                time: '1512916094279146000'
            });

            expect(wallet).to.deep.equal({
                "balance": "40",
                "history_hash": "1818a63b9eca2d82829ddb709b08ee23be0edc30d09a0ad1c09016b09018ea9f",
                "history_len": "3",
                "login": "abcd",
                "pub_key": "2fc4c29755938792d36382adbe2850026e266eb256b125c756e784eaec29174f"
            });

            expect(transactions.length).to.equal(3);

            done();
        });

        server.respond();
    });

});
