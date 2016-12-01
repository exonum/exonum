$(() => {

    // Fetch the button you are using to initiate the PayPal flow
    var paypalButton = document.getElementById('paypal-button');

    // Create a Client component
    braintree.client.create({
        authorization: 'TOKEN'
    }, function(clientErr, clientInstance) {
        // Create PayPal component
        braintree.paypal.create({
            client: clientInstance
        }, function(err, paypalInstance) {
            paypalButton.addEventListener('click', function() {
                // Tokenize here!
                paypalInstance.tokenize({
                    flow: 'checkout', // Required
                    amount: 1.00, // Required
                    currency: 'USD', // Required
                    locale: 'en_US'
                }, function(err, tokenizationPayload) {
                    // Tokenization complete
                    // Send tokenizationPayload.nonce to server
                });
            });
        });
    });

});