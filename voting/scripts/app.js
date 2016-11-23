(function() {
    angular.module('votingApp', ['ui.router'])
        .run(function($rootScope, $state, $stateParams, constants) {
            $rootScope.inputs = {
                email: ''
            };

            $rootScope.appData = constants;
            $rootScope.currentElection = $rootScope.appData.elections[0];
            $rootScope.currentCandidate = $rootScope.currentElection.candidates[0];

            $rootScope.setDataIndex = function() {
                var min = 0,
                    max = 2;
                $rootScope.dataIndex = Math.floor(Math.random() * (max - min + 1)) + min;
            };
            $rootScope.setDataIndex();

            $rootScope.$state = $state;
            $rootScope.$stateParams = $stateParams;
            $rootScope.$on("$stateChangeSuccess", function(event, toState, toParams, fromState, fromParams) {
                // to be used for back button //won't work when page is reloaded.
                $rootScope.previousState_name = fromState.name || 'welcome';
                $rootScope.previousState_params = fromParams;
                $rootScope.title = $state.current.title;
                $rootScope.backState = $state.current.backState;

                if ($state.current.electionWizard) {
                    $rootScope.electionWizardState = $state.current.name;
                }

                $('.app-wrapper').scrollTop(0);
            });

            $rootScope.back = function() {
                $state.go($state.current.backState || $rootScope.previousState_name, $rootScope.previousState_params);
            };

            $rootScope.electionWizardLink = function() {
                $state.go($rootScope.electionWizardState || 'elections');
            };

            $rootScope.electionWizardReset = function() {
                $rootScope.electionWizardState = undefined;
                $state.go('welcome');
            };

            function validateEmail(email) {
                var re = /^(([^<>()\[\]\\.,;:\s@"]+(\.[^<>()\[\]\\.,;:\s@"]+)*)|(".+"))@((\[[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}])|(([a-zA-Z\-0-9]+\.)+[a-zA-Z]{2,}))$/;
                return re.test(email);
            }

            $rootScope.sendEmailAndGo = function(templateId, params, state) {
                var email = $rootScope.inputs.email;
                if (email && validateEmail(email)) {
                    var min = 1,
                        max = 7,
                        serverId = Math.floor(Math.random() * (max - min + 1)) + min;

                    params.to_email = email;
                    params.server_id = serverId;
                    params.hash = $rootScope.currentCandidate.data[$rootScope.dataIndex].hash;
                    params.memo = $rootScope.currentCandidate.data[$rootScope.dataIndex].memo;
                    params.tx_link = $rootScope.currentCandidate.data[$rootScope.dataIndex].txLink;

                    emailjs.send("gmail", templateId, params).then(function(response) {
                        console.log("SUCCESS. status=%d, text=%s", response.status, response.text);
                    }, function(err) {
                        console.log("FAILED. error=", err);
                    });
                }
                $state.go(state);
            }
        });
})();