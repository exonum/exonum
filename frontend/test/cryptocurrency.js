var NETWORK_ID = 0;
var PROTOCOL_VERSION = 0;
var SERVICE_ID = 128;

describe('Configure service', function() {

    it('should correctly initialize if valid configuration is passed', function() {
        var CONFIGURATION = {
            network_id: NETWORK_ID,
            protocol_version: PROTOCOL_VERSION,
            service_id: SERVICE_ID,
            validators: [
                '756f0bb877333e4059e785e38d72b716a2ae9981011563cf21e60ab16bec1fbc',
                '6ce6f6501a03728d25533baf867312d6f425f48c07a1bed669b0afad5d0c136c',
                '8917ecf39f4dc7c5289b4b9a3331c4455fcb1671b47bde39e0ea9361c5752451',
                'a2dda8436715e8fdf6a5f865d5bdbe70b0ffb1d6267352e69a169aa6d8d368fb'
            ]
        };
        var service = new CryptocurrencyService(CONFIGURATION);
        expect(service.configuration).to.deep.equal(CONFIGURATION);
    });

});

describe('Verify API requests', function() {

    var CONFIGURATION = {
        network_id: NETWORK_ID,
        protocol_version: PROTOCOL_VERSION,
        service_id: SERVICE_ID,
        validators: [
            '756f0bb877333e4059e785e38d72b716a2ae9981011563cf21e60ab16bec1fbc',
            '6ce6f6501a03728d25533baf867312d6f425f48c07a1bed669b0afad5d0c136c',
            '8917ecf39f4dc7c5289b4b9a3331c4455fcb1671b47bde39e0ea9361c5752451',
            'a2dda8436715e8fdf6a5f865d5bdbe70b0ffb1d6267352e69a169aa6d8d368fb'
        ]
    };
    var service = new CryptocurrencyService(CONFIGURATION);
    var transactionUrl = 'api/services/cryptocurrency/v1/wallets/transaction';
    var server;

    beforeEach(function() {
        server = sinon.fakeServer.create();

        sinon.spy(jQuery, 'ajax');
    });

    afterEach(function() {
        jQuery.ajax.restore();

        server.restore();
    });

    describe('Create Wallet', function() {

        var publicKey = '03e657ae71e51be60a45b4bd20bcf79ff52f0c037ae6da0540a0e0066132b472';
        var secretKey = '2ef1f2e2799c93b50d2a7ba207a4efebebf6fe5735339dc782e06b1c30b72abf03e657ae71e51be60a45b4bd20bcf79ff52f0c037ae6da0540a0e0066132b472';
        var name = 'John Doe';
        var response = {
            tx_hash: '8616c02c8e5d74df7f6804edcc2e9980ab56c9fd660cb66a737e54f3e2b5eddf'
        };

        it('should sign and submit transaction', function() {
            server.respondWith('POST', transactionUrl, [200, {'Content-Type': 'application/json'}, JSON.stringify(response)]);

            service.createWallet(publicKey, name, secretKey);

            server.respond();

            var spyCall = $.ajax.getCall(0);

            expect(JSON.parse(spyCall.args[0].data)).to.deep.equal({
                body: {
                    pub_key: publicKey,
                    name: name
                },
                network_id: NETWORK_ID,
                protocol_version: PROTOCOL_VERSION,
                service_id: SERVICE_ID,
                message_id: 130,
                signature: 'd62d9f5098707d4cce16e021f6c15399bcb6ff5359dd343b91d8a755b6eba76332328061234a95281d660333842d3f213d054464e20eec43c6b395d468842e0b'
            });

            expect(JSON.parse(spyCall.returnValue.responseText)).to.deep.equal(response);
        });

        it('should throw error on server error', function() {
            server.respondWith('POST', transactionUrl, [404, {'Content-Type': 'application/json'}, '']);

            service.createWallet(publicKey, name, secretKey);

            expect(function(){
                server.respond();
            }).to.throw();
        });

        it('should throw error on wrong format of server response', function() {
            server.respondWith('POST', transactionUrl, [200, {'Content-Type': 'application/json'}, '']);

            service.createWallet(publicKey, name, secretKey);

            expect(function(){
                server.respond();
            }).to.throw();
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

    var service;
    var url = 'api/services/cryptocurrency/v1/wallets/info?pubkey=';

    before(function(done) {
        $.ajax({
            url: 'test_data/validators.json',
            method: 'GET',
            success: function(response) {
                var CONFIGURATION = {
                    network_id: NETWORK_ID,
                    protocol_version: PROTOCOL_VERSION,
                    service_id: SERVICE_ID,
                    validators: response
                };
                service = new CryptocurrencyService(CONFIGURATION);
                done();
            }
        });
    });

    it('should return expected parameters on valid wallet query validation', function(done) {
        var publicKey = '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a';
        var txs = [
            {
                'body': {
                    'name': 'Jane Doe',
                    'pub_key': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a'
                },
                'message_id': 130,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': 'f2c9575b32221cba2de2fe38d37b8ceac312eb74e2bb5ec50e455af39411052fb829846f89f9a724dcd224804e99b768ba74611218d205e470d4075637fc2700',
                'execution_status': true,
                'tx_hash': '097b9c36f115cd415aabf6c6c5861792b54319e2038c0a6239e31bb75a21b8b4'
            },
            {
                'body': {
                    'amount': '6000',
                    'seed': '1000',
                    'wallet': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a'
                },
                'message_id': 129,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': '1e6caf3290b33efd2b62ddbc190fb2793de2b52ca1ce2f6e518779f141e0c66f1530667d0f859ac6a0743f40532d97497b011cf1cf3e0fa99a9e1a459618c208',
                'execution_status': true,
                'tx_hash': '3012664984624c846f10f11fe41db9498167aa7c1feab0cd13633954589b9497'
            },
            {
                'body': {
                    'amount': '3000',
                    'from': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a',
                    'seed': '2000',
                    'to': '0b513ad9b4924015ca0902ed079044d3ac5dbec2306f06948c10da8eb6e39f2d'
                },
                'message_id': 128,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': '51a142df1654bf28a3bb251ca4b1c18b78c569dbfdb29b7f2b2e70d43295d1a3731cf0dd1da15c8bf39abd9c7d84791edb4a592423870a1fe94b9ff2cc22c405',
                'execution_status': true,
                'tx_hash': '48ce786b1fd365038836cb512ce22d589f6896c3188af1f0841cf06d869c6b59'
            },
            {
                'body': {
                    'amount': '1000',
                    'from': '0b513ad9b4924015ca0902ed079044d3ac5dbec2306f06948c10da8eb6e39f2d',
                    'seed': '3000',
                    'to': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a'
                },
                'message_id': 128,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': 'de3164d461ee455c3360bdf54e3d0004f8c1dbe7e6371787c328710db55243bb77e77a7e71bdd64ca3ad205acf95d8d1cdb7e2c37f95b91c8ddaa9f1337de806',
                'execution_status': true,
                'tx_hash': 'c44759951d9cfc30de58e891bd842efec8e11d38fdbfb4875fb93ad0f3752fb2'
            }
        ];

        $.ajax({
            url: 'test_data/wallet1_query.json',
            method: 'GET',
            success: function(response) {
                var server = sinon.fakeServer.create();

                sinon.spy(jQuery, 'ajax');

                server.respondWith('GET', url + publicKey, [200, {'Content-Type': 'application/json'}, JSON.stringify(response)]);

                service.getWallet(publicKey, function(error, block, wallet, transactions) {
                    expect(block).to.deep.equal({
                        'height': '4',
                        'prev_hash': '2e933eba2887a1d9bb38c396577be23db58ea5f414761f6dda939d660b323140',
                        'proposer_id': 0,
                        'schema_version': 0,
                        'state_hash': 'da5ae8362137d3e4acae0917e30388959b6d2a91760d25bb5eca832b449550ce',
                        'tx_count': 1,
                        'tx_hash': '759de4b2df16488e1c13c20cb9a356487204abcedd97177f2fe773c187beb29e',
                        'time': '0'
                    });

                    expect(wallet).to.deep.equal({
                        'balance': '4000',
                        'history_hash': 'b83df3e53e8623884024c72e3bcc6c5251b1ee7fc1ff2682464e53f58eb61de7',
                        'history_len': '4',
                        'name': 'Jane Doe',
                        'pub_key': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a'
                    });

                    expect(transactions.length).to.equal(2);

                    for (var i = 0; i < transactions.length; i++) {
                        expect(transactions[i]).to.deep.equal(txs[i]);
                    }

                    jQuery.ajax.restore();

                    server.restore();

                    done();
                });

                server.respond();
            }
        });
    });

    it('should return expected parameters on valid wallet query validation', function(done) {
        var publicKey = '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a';
        var txs = [
            {
                'body': {
                    'name': 'Jane Doe',
                    'pub_key': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a'
                },
                'message_id': 130,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': 'f2c9575b32221cba2de2fe38d37b8ceac312eb74e2bb5ec50e455af39411052fb829846f89f9a724dcd224804e99b768ba74611218d205e470d4075637fc2700',
                'execution_status': true,
                'tx_hash': '097b9c36f115cd415aabf6c6c5861792b54319e2038c0a6239e31bb75a21b8b4'
            },
            {
                'body': {
                    'amount': '6000',
                    'seed': '1000',
                    'wallet': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a'
                },
                'message_id': 129,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': '1e6caf3290b33efd2b62ddbc190fb2793de2b52ca1ce2f6e518779f141e0c66f1530667d0f859ac6a0743f40532d97497b011cf1cf3e0fa99a9e1a459618c208',
                'execution_status': true,
                'tx_hash': '3012664984624c846f10f11fe41db9498167aa7c1feab0cd13633954589b9497'
            }
        ];

        $.ajax({
            url: 'test_data/wallet1_query1.json',
            method: 'GET',
            success: function(response) {
                var server = sinon.fakeServer.create();

                sinon.spy(jQuery, 'ajax');

                server.respondWith('GET', url + publicKey, [200, {'Content-Type': 'application/json'}, JSON.stringify(response)]);

                service.getWallet(publicKey, function(error, block, wallet, transactions) {
                    expect(block).to.deep.equal({
                        'height': '2',
                        'prev_hash': '4c1542370f9b97bfe99e671b68fc970317e9b0dfa25a8fc23856da59a1e35b2a',
                        'proposer_id': 0,
                        'schema_version': 0,
                        'state_hash': 'c73c4b61b05865db98d77db22032fe35c174775c57faa6f5c5e0b430bae3e6ed',
                        'tx_count': 1,
                        'tx_hash': 'de134bb9ad5c643f2cc57f4fce5f97a93bb8aaabac5197b5f72136df88171299',
                        'time': '0'
                    });

                    expect(wallet).to.deep.equal({
                        'balance': '6000',
                        'history_hash': '12edfa5a5508993c0b3d2adf142a3e0042a9607d5dd831689c04f200e6682cd9',
                        'history_len': '2',
                        'name': 'Jane Doe',
                        'pub_key': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a'
                    });

                    expect(transactions.length).to.equal(4);

                    for (var i = 0; i < transactions.length; i++) {
                        expect(transactions[i]).to.deep.equal(txs[i]);
                    }

                    jQuery.ajax.restore();

                    server.restore();

                    done();
                });

                server.respond();
            }
        });
    });

    it('should return expected parameters on valid wallet query validation', function(done) {
        var publicKey = '0b513ad9b4924015ca0902ed079044d3ac5dbec2306f06948c10da8eb6e39f2d';
        var txs = [
            {
                'body': {
                    'name': 'Dillinger Escape Plan',
                    'pub_key': '0b513ad9b4924015ca0902ed079044d3ac5dbec2306f06948c10da8eb6e39f2d'
                },
                'message_id': 130,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': '16ed1d1fb4402ea6b5d920ef4ea8878e1a6dffc0b120542c5c7834212553c2a838df4857be061b5c6cda326e78698ff8d73d8a66576507850337df5dd4fb5107',
                'execution_status': true,
                'tx_hash': '334045918808103ea8d23b9f31628cb265ebcdb4ef3f60ea9788ddca76aa7b78'
            },
            {
                'body': {
                    'amount': '3000',
                    'from': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a',
                    'seed': '2000',
                    'to': '0b513ad9b4924015ca0902ed079044d3ac5dbec2306f06948c10da8eb6e39f2d'
                },
                'message_id': 128,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': '51a142df1654bf28a3bb251ca4b1c18b78c569dbfdb29b7f2b2e70d43295d1a3731cf0dd1da15c8bf39abd9c7d84791edb4a592423870a1fe94b9ff2cc22c405',
                'execution_status': true,
                'tx_hash': '48ce786b1fd365038836cb512ce22d589f6896c3188af1f0841cf06d869c6b59'
            },
            {
                'body': {
                    'amount': '1000',
                    'from': '0b513ad9b4924015ca0902ed079044d3ac5dbec2306f06948c10da8eb6e39f2d',
                    'seed': '3000',
                    'to': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a'
                },
                'message_id': 128,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': 'de3164d461ee455c3360bdf54e3d0004f8c1dbe7e6371787c328710db55243bb77e77a7e71bdd64ca3ad205acf95d8d1cdb7e2c37f95b91c8ddaa9f1337de806',
                'execution_status': true,
                'tx_hash': 'c44759951d9cfc30de58e891bd842efec8e11d38fdbfb4875fb93ad0f3752fb2'
            }
        ];

        $.ajax({
            url: 'test_data/wallet2_query.json',
            method: 'GET',
            success: function(response) {
                var server = sinon.fakeServer.create();

                sinon.spy(jQuery, 'ajax');

                server.respondWith('GET', url + publicKey, [200, {'Content-Type': 'application/json'}, JSON.stringify(response)]);

                service.getWallet(publicKey, function(error, block, wallet, transactions) {
                    expect(block).to.deep.equal({
                        'height': '4',
                        'prev_hash': '2e933eba2887a1d9bb38c396577be23db58ea5f414761f6dda939d660b323140',
                        'proposer_id': 0,
                        'schema_version': 0,
                        'state_hash': 'da5ae8362137d3e4acae0917e30388959b6d2a91760d25bb5eca832b449550ce',
                        'tx_count': 1,
                        'tx_hash': '759de4b2df16488e1c13c20cb9a356487204abcedd97177f2fe773c187beb29e',
                        'time': '0'
                    });

                    expect(wallet).to.deep.equal({
                        'balance': '2000',
                        'history_hash': '207799f4a3fa412890614d6c513c82ad2bd4ffca5fd9d5392b68b1a8c85d7e6c',
                        'history_len': '3',
                        'name': 'Dillinger Escape Plan',
                        'pub_key': '0b513ad9b4924015ca0902ed079044d3ac5dbec2306f06948c10da8eb6e39f2d'
                    });

                    expect(transactions.length).to.equal(3);

                    for (var i = 0; i < transactions.length; i++) {
                        expect(transactions[i]).to.deep.equal(txs[i]);
                    }

                    jQuery.ajax.restore();

                    server.restore();

                    done();
                });

                server.respond();
            }
        });
    });

    it('should return expected parameters on valid wallet query validation with tx of false execution status', function(done) {
        var publicKey = '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a';
        var txs = [
            {
                'body': {
                    'name': 'Jane Doe',
                    'pub_key': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a'
                },
                'message_id': 130,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': 'f2c9575b32221cba2de2fe38d37b8ceac312eb74e2bb5ec50e455af39411052fb829846f89f9a724dcd224804e99b768ba74611218d205e470d4075637fc2700',
                'execution_status': true,
                'tx_hash': '097b9c36f115cd415aabf6c6c5861792b54319e2038c0a6239e31bb75a21b8b4'
            },
            {
                'body': {
                    'amount': '6000',
                    'seed': '1000',
                    'wallet': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a'
                },
                'message_id': 129,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': '1e6caf3290b33efd2b62ddbc190fb2793de2b52ca1ce2f6e518779f141e0c66f1530667d0f859ac6a0743f40532d97497b011cf1cf3e0fa99a9e1a459618c208',
                'execution_status': true,
                'tx_hash': '3012664984624c846f10f11fe41db9498167aa7c1feab0cd13633954589b9497'
            },
            {
                'body': {
                    'amount': '3000',
                    'from': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a',
                    'seed': '2000',
                    'to': '0b513ad9b4924015ca0902ed079044d3ac5dbec2306f06948c10da8eb6e39f2d'
                },
                'message_id': 128,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': '51a142df1654bf28a3bb251ca4b1c18b78c569dbfdb29b7f2b2e70d43295d1a3731cf0dd1da15c8bf39abd9c7d84791edb4a592423870a1fe94b9ff2cc22c405',
                'execution_status': true,
                'tx_hash': '48ce786b1fd365038836cb512ce22d589f6896c3188af1f0841cf06d869c6b59'
            },
            {
                'body': {
                    'amount': '1000',
                    'from': '0b513ad9b4924015ca0902ed079044d3ac5dbec2306f06948c10da8eb6e39f2d',
                    'seed': '3000',
                    'to': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a'
                },
                'message_id': 128,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': 'de3164d461ee455c3360bdf54e3d0004f8c1dbe7e6371787c328710db55243bb77e77a7e71bdd64ca3ad205acf95d8d1cdb7e2c37f95b91c8ddaa9f1337de806',
                'execution_status': true,
                'tx_hash': 'c44759951d9cfc30de58e891bd842efec8e11d38fdbfb4875fb93ad0f3752fb2'
            },
            {
                'body': {
                    'name': 'Change name of existing wallet',
                    'pub_key': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a'
                },
                'message_id': 130,
                'network_id': 0,
                'protocol_version': 0,
                'service_id': 128,
                'signature': 'ebbb8335103dbfabde282c7fc3ef8a01d2dee93d6da3179982cb3e690ed43cfb3dbfeae69c27fd9bc517760fbb260ea9ec8ff8ea86e5834a1308003f49e73b0d',
                'execution_status': false,
                'tx_hash': 'dac9d0dd7ca71ad85a14a9f189f31b3ba534005b26f95091b60119da38714b62'
            }
        ];

        $.ajax({
            url: 'test_data/tx_create_wallet_false_execution_status.json',
            method: 'GET',
            success: function(response) {
                var server = sinon.fakeServer.create();

                sinon.spy(jQuery, 'ajax');

                server.respondWith('GET', url + publicKey, [200, {'Content-Type': 'application/json'}, JSON.stringify(response)]);

                service.getWallet(publicKey, function(error, block, wallet, transactions) {
                    expect(block).to.deep.equal({
                        'height': '5',
                        'prev_hash': '1a1b6bf4c9f7543809e1011b1d5e4ad0b76eab14924d8ff00ba1a79f0466ce6b',
                        'proposer_id': 0,
                        'schema_version': 0,
                        'state_hash': 'e637fdd5e748f44be52a89d8ace6c1da54cc97f0ebdb53a9e8ab04b17eaa2a2f',
                        'tx_count': 1,
                        'tx_hash': 'cb63c0d72909e1f51be6601df23aa1a5d291aec1f13dc7f65997a7cbd97899ba',
                        'time': '0'
                    });

                    expect(wallet).to.deep.equal({
                        'balance': '4000',
                        'history_hash': 'af70b9f0e5937e3ef8b80e39d733a81bab01a495afcd1c960552f81b213d22b8',
                        'history_len': '5',
                        'name': 'Jane Doe',
                        'pub_key': '66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a'
                    });

                    expect(transactions.length).to.equal(5);

                    for (var i = 0; i < transactions.length; i++) {
                        expect(transactions[i]).to.deep.equal(txs[i]);
                    }

                    jQuery.ajax.restore();

                    server.restore();

                    done();
                });

                server.respond();
            }
        });
    });

    it('should return undefined on absent wallet', function(done) {
        var publicKey = 'bdbbc4edb3f589728bb954f10867332ffd9dac8e933fc6b3607ef552e4ed84d3';

        $.ajax({
            url: 'test_data/response_absent_wallet.json',
            method: 'GET',
            success: function(response) {
                var server = sinon.fakeServer.create();

                sinon.spy(jQuery, 'ajax');

                server.respondWith('GET', url + publicKey, [200, {'Content-Type': 'application/json'}, JSON.stringify(response)]);

                service.getWallet(publicKey, function(error, block, wallet, transactions) {
                    expect(wallet).to.equal(undefined);

                    jQuery.ajax.restore();

                    server.restore();

                    done();
                });

                server.respond();
            }
        });
    });

});
